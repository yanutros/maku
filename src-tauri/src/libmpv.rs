//! Linux : le film dessiné dans la fenêtre de Maku par libmpv.
//!
//! Sous Windows, mpv dessine dans la fenêtre de Maku (`--wid`). Sous Linux, un programme ne dessine pas dans la
//! fenêtre d'un autre de cette façon, et deux fenêtres ne tiennent pas ensemble hors du plein écran (voir `overlay`).
//! Ici, mpv n'est donc plus un programme à part : sa bibliothèque (libmpv) est chargée dans Maku et dessine, par son
//! API de rendu OpenGL, dans une zone placée sous la page web, dont le fond devient transparent pendant le film. Une
//! seule fenêtre : en fenêtré comme en plein écran, sous X11 comme sous Wayland.
//!
//! La bibliothèque est chargée à l'exécution (`dlopen`) : son numéro diffère d'une distribution à l'autre, et un
//! paquet construit sur l'une doit tourner sur les autres. Absente ou inutilisable, Maku lance mpv comme avant.
//!
//! L'instance embarquée ouvre le même canal JSON que le programme mpv (`input-ipc-server`) : pause, pistes,
//! progression et épisode suivant passent par le suivi de `player.rs`, sans rien de particulier.

use anyhow::{anyhow, bail, Context, Result};
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::ffi::{c_char, c_int, c_void, CString};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use tauri::{AppHandle, Manager};
use webkit2gtk::WebViewExt;

type Ptr = *mut c_void;

/// Variable d'environnement qui garde mpv dans sa propre fenêtre, comme avant (« 1 »).
pub const OPT_OUT: &str = "MAKU_NO_LIBMPV";
/// Modes de rendu de repli de WebKitGTK : la page n'y est pas dessinée au-dessus de l'image (constaté avec
/// WebKitGTK 2.52). Le film serait à l'écran sans aucune commande : mpv garde alors sa fenêtre.
const WEBKIT_FALLBACKS: [&str; 2] = ["WEBKIT_DISABLE_DMABUF_RENDERER", "WEBKIT_DISABLE_COMPOSITING_MODE"];

/// Une lecture dans la fenêtre a échoué (pas de contexte OpenGL, libmpv trop ancienne…) : les suivantes reprennent
/// la fenêtre de mpv, jusqu'à la fermeture de Maku.
static FAILED: AtomicBool = AtomicBool::new(false);

/// L'environnement permet-il de dessiner le film dans la fenêtre ? `var` lit une variable d'environnement.
pub fn allowed(var: impl Fn(&str) -> Option<String>) -> bool {
    let set = |name: &str| var(name).is_some_and(|v| !matches!(v.trim(), "" | "0"));
    !set(OPT_OUT) && !WEBKIT_FALLBACKS.iter().any(|name| set(name))
}

/// Le film peut-il être dessiné dans la fenêtre de Maku ? Sinon, mpv ouvre la sienne.
pub fn usable() -> bool {
    !FAILED.load(Ordering::Relaxed) && allowed(|name| std::env::var(name).ok()) && api().is_some()
}

/// Où va le film : dans la fenêtre de Maku par libmpv (premier), ou dans la fenêtre de mpv placée sous celle de Maku
/// (second, le réglage expérimental `overlay`). libmpv utilisable, elle l'emporte : ce mode-là ne tient qu'en plein
/// écran. Ni l'un ni l'autre : mpv ouvre sa fenêtre, avec ses commandes.
pub fn modes(usable: bool, overlay_setting: bool) -> (bool, bool) {
    (usable, overlay_setting && !usable)
}

/// Ce que libmpv reçoit, tiré de la ligne de commande préparée pour le programme mpv (`player::prepare`) : les mêmes
/// réglages, pour que le film se lise de la même façon.
#[derive(Debug, PartialEq)]
pub struct Plan {
    /// Options (nom, valeur) à poser avant le démarrage.
    pub options: Vec<(String, String)>,
    /// Fichier de liste à charger ensuite : pour le programme, c'est un argument et non une option.
    pub playlist: Option<String>,
}

/// Traduit les arguments de mpv (« --nom=valeur », « --nom ») en options de libmpv. Ce qui concerne la fenêtre de
/// mpv est écarté : il n'y en a plus. Un argument qui n'est pas une option (un fichier) est refusé : l'adresse du
/// film ne passe que par le fichier de liste.
pub fn plan(args: &[String]) -> Result<Plan> {
    const WINDOW_ONLY: [&str; 3] = ["fullscreen", "geometry", "force-window"];
    // Le rendu dans la zone de Maku, et non dans une fenêtre de mpv.
    let mut plan = Plan { options: vec![("vo".into(), "libmpv".into())], playlist: None };
    for arg in args {
        let option = arg.strip_prefix("--").filter(|o| !o.is_empty()).ok_or_else(|| anyhow!("argument de mpv inattendu"))?;
        let (name, value) = option.split_once('=').unwrap_or((option, "yes"));
        match name {
            "playlist" => plan.playlist = Some(value.to_owned()),
            // Le programme mpv reçoit sa liste au lancement et s'arrête à la fin (« idle=no »). La bibliothèque la
            // reçoit après son démarrage : sans rien à lire, « no » l'arrêterait aussitôt. « once » attend la
            // liste, puis s'arrête de même à la fin du film.
            "idle" if value == "no" => plan.options.push(("idle".into(), "once".into())),
            "vo" | "wid" => bail!("option de mpv inattendue : {name}"),
            _ if WINDOW_ONLY.contains(&name) => {}
            _ => plan.options.push((name.to_owned(), value.to_owned())),
        }
    }
    Ok(plan)
}

#[repr(C)]
struct Param {
    kind: c_int,
    data: *mut c_void,
}

/// `mpv_opengl_init_params`. Le troisième champ n'existe que dans l'API 1 de libmpv, qui le lit : il reste nul.
#[repr(C)]
struct GlInit {
    get_proc_address: unsafe extern "C" fn(Ptr, *const c_char) -> Ptr,
    ctx: Ptr,
    extra_exts: *const c_char,
}

/// `mpv_opengl_fbo`
#[repr(C)]
struct Fbo {
    fbo: c_int,
    w: c_int,
    h: c_int,
    internal_format: c_int,
}

const PARAM_API_TYPE: c_int = 1;
const PARAM_OPENGL_INIT: c_int = 2;
const PARAM_OPENGL_FBO: c_int = 3;
const PARAM_FLIP_Y: c_int = 4;
const PARAM_X11_DISPLAY: c_int = 8;
const PARAM_WL_DISPLAY: c_int = 9;

/// Les fonctions de libmpv dont Maku se sert.
struct Api {
    create: unsafe extern "C" fn() -> Ptr,
    set_option_string: unsafe extern "C" fn(Ptr, *const c_char, *const c_char) -> c_int,
    initialize: unsafe extern "C" fn(Ptr) -> c_int,
    command: unsafe extern "C" fn(Ptr, *const *const c_char) -> c_int,
    terminate_destroy: unsafe extern "C" fn(Ptr),
    render_create: unsafe extern "C" fn(*mut Ptr, Ptr, *const Param) -> c_int,
    render_set_update: unsafe extern "C" fn(Ptr, Option<unsafe extern "C" fn(Ptr)>, Ptr),
    render: unsafe extern "C" fn(Ptr, *const Param) -> c_int,
    render_free: unsafe extern "C" fn(Ptr),
    _lib: libloading::Library,
}

/// Noms sous lesquels les distributions installent libmpv (API 2 depuis mpv 0.35, API 1 avant).
const LIBMPV: [&str; 3] = ["libmpv.so.2", "libmpv.so.1", "libmpv.so"];

fn api() -> Option<&'static Api> {
    static API: OnceLock<Option<Api>> = OnceLock::new();
    API.get_or_init(|| LIBMPV.iter().find_map(|name| unsafe { open_library(name) })).as_ref()
}

unsafe fn open_library(name: &str) -> Option<Api> {
    let lib = libloading::Library::new(name).ok()?;
    macro_rules! sym {
        ($name:literal) => {
            *lib.get(concat!($name, "\0").as_bytes()).ok()?
        };
    }
    Some(Api {
        create: sym!("mpv_create"),
        set_option_string: sym!("mpv_set_option_string"),
        initialize: sym!("mpv_initialize"),
        command: sym!("mpv_command"),
        terminate_destroy: sym!("mpv_terminate_destroy"),
        render_create: sym!("mpv_render_context_create"),
        render_set_update: sym!("mpv_render_context_set_update_callback"),
        render: sym!("mpv_render_context_render"),
        render_free: sym!("mpv_render_context_free"),
        _lib: lib,
    })
}

/// Les fonctions OpenGL, cherchées dans le contexte que GTK a créé : EGL (Wayland) ou GLX (X11).
struct Gl {
    egl: Option<unsafe extern "C" fn(*const c_char) -> Ptr>,
    glx: Option<unsafe extern "C" fn(*const c_char) -> Ptr>,
    use_egl: AtomicBool,
    _libs: Vec<libloading::Library>,
}

static GL: OnceLock<Gl> = OnceLock::new();

/// À appeler avec le contexte OpenGL de la zone actif.
unsafe fn gl() -> &'static Gl {
    let gl = GL.get_or_init(|| {
        let (mut egl, mut glx, mut current, mut libs) = (None, None, None::<unsafe extern "C" fn() -> Ptr>, Vec::new());
        if let Ok(lib) = libloading::Library::new("libEGL.so.1") {
            egl = lib.get(b"eglGetProcAddress\0").ok().map(|s| *s);
            current = lib.get(b"eglGetCurrentContext\0").ok().map(|s| *s);
            libs.push(lib);
        }
        if let Ok(lib) = libloading::Library::new("libGL.so.1") {
            glx = lib.get(b"glXGetProcAddressARB\0").ok().map(|s| *s);
            libs.push(lib);
        }
        let use_egl = egl.is_some() && current.is_some_and(|f| !f().is_null());
        Gl { egl, glx, use_egl: AtomicBool::new(use_egl), _libs: libs }
    });
    gl
}

unsafe extern "C" fn proc_address(_ctx: Ptr, name: *const c_char) -> Ptr {
    let Some(gl) = GL.get() else { return std::ptr::null_mut() };
    let find = if gl.use_egl.load(Ordering::Relaxed) { gl.egl } else { gl.glx };
    find.map_or(std::ptr::null_mut(), |f| f(name))
}

/// L'affichage du bureau (X11 ou Wayland) tel que libmpv l'attend, pour décoder la vidéo avec la carte graphique :
/// le type du paramètre de rendu et le pointeur. GTK les donne par des fonctions propres à chaque bureau.
fn native_display(area: &gtk::GLArea) -> Option<(c_int, Ptr)> {
    use gtk::glib::translate::ToGlibPtr;
    let display = area.display();
    let (kind, getter): (c_int, &[u8]) = match display.type_().name() {
        "GdkX11Display" => (PARAM_X11_DISPLAY, b"gdk_x11_display_get_xdisplay\0"),
        "GdkWaylandDisplay" => (PARAM_WL_DISPLAY, b"gdk_wayland_display_get_wl_display\0"),
        _ => return None,
    };
    static GDK: OnceLock<Option<libloading::Library>> = OnceLock::new();
    let gdk = GDK.get_or_init(|| unsafe { libloading::Library::new("libgdk-3.so.0") }.ok()).as_ref()?;
    let raw: *mut gtk::gdk::ffi::GdkDisplay = display.to_glib_none().0;
    let native = unsafe { (*gdk.get::<unsafe extern "C" fn(*mut gtk::gdk::ffi::GdkDisplay) -> Ptr>(getter).ok()?)(raw) };
    (!native.is_null()).then_some((kind, native))
}

/// La zone de dessin placée sous la page, dans la fenêtre de Maku. Elle n'existe que sur le fil de GTK.
struct Surface {
    area: gtk::GLArea,
    webview: webkit2gtk::WebView,
    /// Rendu de la lecture en cours (`mpv_render_context`), nul entre deux films.
    render: Cell<Ptr>,
    /// Fond de la page avant le film, remis ensuite.
    background: Cell<Option<gtk::gdk::RGBA>>,
}

thread_local! {
    static SURFACE: RefCell<Option<Rc<Surface>>> = const { RefCell::new(None) };
}

/// Une nouvelle image attend déjà d'être dessinée.
static REDRAW: AtomicBool = AtomicBool::new(false);

/// libmpv a une nouvelle image (appelé depuis ses fils) : la zone est redessinée sur le fil de GTK.
unsafe extern "C" fn on_update(_ctx: Ptr) {
    if !REDRAW.swap(true, Ordering::AcqRel) {
        gtk::glib::MainContext::default().invoke(|| {
            REDRAW.store(false, Ordering::Release);
            SURFACE.with(|s| {
                if let Some(surface) = s.borrow().as_ref() {
                    surface.area.queue_render();
                }
            });
        });
    }
}

impl Surface {
    /// Glisse une zone OpenGL sous la page : la page quitte la boîte où Tauri l'a rangée pour une pile (`GtkOverlay`)
    /// mise à la même place, la zone dessous et la page dessus. Fait une fois, à la première lecture.
    fn install(webview: webkit2gtk::WebView) -> Result<Rc<Surface>> {
        let parent = webview.parent().and_then(|p| p.downcast::<gtk::Box>().ok()).context("la page n'est pas dans la boîte attendue")?;
        let position = parent.child_position(&webview);
        let stack = gtk::Overlay::new();
        let area = gtk::GLArea::new();
        parent.remove(&webview);
        stack.add(&area);
        stack.add_overlay(&webview);
        parent.pack_start(&stack, true, true, 0);
        parent.reorder_child(&stack, position);
        stack.show_all();
        area.realize();

        let surface = Rc::new(Surface { area, webview, render: Cell::new(std::ptr::null_mut()), background: Cell::new(None) });
        let weak = Rc::downgrade(&surface);
        surface.area.connect_render(move |area, _| {
            if let (Some(surface), Some(api)) = (weak.upgrade(), api()) {
                unsafe { surface.draw(api, area) };
            }
            gtk::glib::Propagation::Stop
        });
        // Fenêtre fermée pendant un film : le rendu est libéré tant que son contexte OpenGL existe.
        let weak = Rc::downgrade(&surface);
        surface.area.connect_unrealize(move |_| {
            if let Some(surface) = weak.upgrade() {
                surface.release();
            }
        });
        Ok(surface)
    }

    unsafe fn draw(&self, api: &Api, area: &gtk::GLArea) {
        const DRAW_FRAMEBUFFER_BINDING: u32 = 0x8CA6;
        const COLOR_BUFFER_BIT: u32 = 0x4000;
        let name = |n: &'static [u8]| proc_address(std::ptr::null_mut(), n.as_ptr().cast());
        let render = self.render.get();
        if render.is_null() {
            // Entre deux films : du noir, que la page (opaque) recouvre.
            let (color, clear) = (name(b"glClearColor\0"), name(b"glClear\0"));
            if !color.is_null() && !clear.is_null() {
                std::mem::transmute::<Ptr, unsafe extern "C" fn(f32, f32, f32, f32)>(color)(0.0, 0.0, 0.0, 1.0);
                std::mem::transmute::<Ptr, unsafe extern "C" fn(u32)>(clear)(COLOR_BUFFER_BIT);
            }
            return;
        }
        // GTK dessine la zone dans un tampon à lui, déjà actif : libmpv doit dessiner dans celui-là.
        let mut target: c_int = 0;
        let get = name(b"glGetIntegerv\0");
        if !get.is_null() {
            std::mem::transmute::<Ptr, unsafe extern "C" fn(u32, *mut c_int)>(get)(DRAW_FRAMEBUFFER_BINDING, &mut target);
        }
        let scale = area.scale_factor();
        let mut fbo = Fbo { fbo: target, w: area.allocated_width() * scale, h: area.allocated_height() * scale, internal_format: 0 };
        let mut flip: c_int = 1;
        let params = [
            Param { kind: PARAM_OPENGL_FBO, data: (&mut fbo as *mut Fbo).cast() },
            Param { kind: PARAM_FLIP_Y, data: (&mut flip as *mut c_int).cast() },
            Param { kind: 0, data: std::ptr::null_mut() },
        ];
        (api.render)(render, params.as_ptr());
    }

    /// Branche le rendu de `mpv` sur la zone, et rend le fond de la page transparent.
    fn attach(&self, api: &Api, mpv: Ptr) -> Result<()> {
        self.release();
        self.area.make_current();
        if let Some(e) = self.area.error() {
            bail!("pas de contexte OpenGL : {e}");
        }
        let mut render: Ptr = std::ptr::null_mut();
        let code = unsafe {
            gl();
            let mut init = GlInit { get_proc_address: proc_address, ctx: std::ptr::null_mut(), extra_exts: std::ptr::null() };
            // Sans l'affichage du bureau, la liste s'arrête un cran plus tôt : la vidéo est décodée sans lui.
            let (display, native) = native_display(&self.area).unwrap_or((0, std::ptr::null_mut()));
            let params = [
                Param { kind: PARAM_API_TYPE, data: c"opengl".as_ptr() as *mut c_void },
                Param { kind: PARAM_OPENGL_INIT, data: (&mut init as *mut GlInit).cast() },
                Param { kind: display, data: native },
                Param { kind: 0, data: std::ptr::null_mut() },
            ];
            (api.render_create)(&mut render, mpv, params.as_ptr())
        };
        if code < 0 || render.is_null() {
            bail!("libmpv refuse le rendu OpenGL (code {code})");
        }
        unsafe { (api.render_set_update)(render, Some(on_update), std::ptr::null_mut()) };
        self.render.set(render);
        self.background.set(Some(self.webview.background_color()));
        self.webview.set_background_color(&gtk::gdk::RGBA::new(0.0, 0.0, 0.0, 0.0));
        self.area.queue_render();
        Ok(())
    }

    /// Fin du film : le rendu est libéré (avant l'arrêt de mpv, comme libmpv le demande) et la page retrouve son fond.
    fn release(&self) {
        let render = self.render.replace(std::ptr::null_mut());
        if !render.is_null() {
            if let Some(api) = api() {
                self.area.make_current();
                unsafe { (api.render_free)(render) };
            }
        }
        if let Some(background) = self.background.take() {
            self.webview.set_background_color(&background);
        }
        if self.area.is_realized() {
            self.area.queue_render();
        }
    }
}

/// Sur le fil de GTK : la zone (créée à la première lecture) reçoit le rendu de `mpv`.
fn attach(webview: webkit2gtk::WebView, mpv: Mpv) -> Result<()> {
    let api = api().context("libmpv absente")?;
    let existing = SURFACE.with(|s| s.borrow().clone());
    let surface = match existing {
        Some(surface) => surface,
        None => {
            let surface = Surface::install(webview)?;
            SURFACE.with(|s| *s.borrow_mut() = Some(surface.clone()));
            surface
        }
    };
    surface.attach(api, mpv.0)
}

/// Pointeur de libmpv, dont les fonctions s'appellent depuis n'importe quel fil.
#[derive(Clone, Copy)]
struct Mpv(Ptr);
unsafe impl Send for Mpv {}
unsafe impl Sync for Mpv {}

fn text(s: &str) -> Result<CString> {
    CString::new(s).context("texte invalide pour mpv")
}

/// Pose les options du plan et démarre mpv (sans rien lire encore).
fn configure(api: &Api, mpv: Mpv, plan: &Plan) -> Result<()> {
    for (name, value) in &plan.options {
        let code = unsafe { (api.set_option_string)(mpv.0, text(name)?.as_ptr(), text(value)?.as_ptr()) };
        if code < 0 {
            bail!("libmpv refuse l'option {name} (code {code})");
        }
    }
    if unsafe { (api.initialize)(mpv.0) } < 0 {
        bail!("libmpv ne s'initialise pas");
    }
    Ok(())
}

/// Charge le fichier de liste du plan : la lecture commence.
fn load(api: &Api, mpv: Mpv, plan: &Plan) -> Result<()> {
    let playlist = text(plan.playlist.as_deref().context("rien à lire")?)?;
    let command = [c"loadlist".as_ptr(), playlist.as_ptr(), std::ptr::null()];
    if unsafe { (api.command)(mpv.0, command.as_ptr()) } < 0 {
        bail!("libmpv refuse la liste de lecture");
    }
    Ok(())
}

/// Un film en cours dans la fenêtre de Maku. `close` l'arrête.
pub struct Playback {
    mpv: Option<Mpv>,
    app: AppHandle,
}

extern "C" {
    fn setlocale(category: c_int, locale: *const c_char) -> *mut c_char;
}
const LC_NUMERIC: c_int = 1;

impl Playback {
    /// Démarre libmpv avec les réglages de `args` (la ligne de commande préparée pour le programme mpv), branche son
    /// image sous la page de la fenêtre principale et lance la lecture.
    pub async fn open(app: &AppHandle, args: &[String]) -> Result<Playback> {
        let result = Self::start(app, args).await;
        if result.is_err() {
            FAILED.store(true, Ordering::Relaxed);
        }
        result
    }

    async fn start(app: &AppHandle, args: &[String]) -> Result<Playback> {
        let api = api().context("libmpv absente")?;
        let plan = plan(args)?;
        let mpv = Mpv(unsafe {
            // libmpv exige des nombres écrits à l'anglaise ; GTK a pris la langue du bureau.
            setlocale(LC_NUMERIC, c"C".as_ptr());
            (api.create)()
        });
        if mpv.0.is_null() {
            bail!("libmpv ne démarre pas");
        }
        // À partir d'ici, tout échec arrête l'instance créée.
        let mut playback = Playback { mpv: Some(mpv), app: app.clone() };
        let started = async {
            configure(api, mpv, &plan)?;
            let window = app.get_webview_window("main").context("fenêtre principale introuvable")?;
            let (done, attached) = tokio::sync::oneshot::channel();
            window.with_webview(move |webview| {
                let _ = done.send(attach(webview.inner(), mpv).map_err(|e| format!("{e:#}")));
            })?;
            attached.await.context("la fenêtre s'est fermée")?.map_err(|e| anyhow!(e))?;
            load(api, mpv, &plan)
        }
        .await;
        match started {
            Ok(()) => Ok(playback),
            Err(e) => {
                playback.close().await;
                Err(e)
            }
        }
    }

    /// Arrête le film : le rendu est d'abord débranché sur le fil de GTK, puis mpv est arrêté. Sans effet la
    /// deuxième fois.
    pub async fn close(&mut self) {
        let Some(mpv) = self.mpv.take() else { return };
        let (done, released) = tokio::sync::oneshot::channel();
        let asked = self.app.run_on_main_thread(move || {
            SURFACE.with(|s| {
                if let Some(surface) = s.borrow().as_ref() {
                    surface.release();
                }
            });
            let _ = done.send(());
        });
        if asked.is_ok() {
            let _ = released.await;
        }
        if let Some(api) = api() {
            // Attend la fin des fils de mpv : hors des fils qui font tourner l'application.
            let _ = tauri::async_runtime::spawn_blocking(move || unsafe { (api.terminate_destroy)({ mpv }.0) })
            .await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|a| a.to_string()).collect()
    }

    #[test]
    fn options_de_libmpv_tirees_de_la_ligne_de_commande() {
        let plan = plan(&args(&["--input-ipc-server=/tmp/s.sock", "--idle=no", "--quiet", "--playlist=/tmp/liste.m3u", "--title=Un film = deux", "--force-window=yes", "--fullscreen", "--geometry=+1200+0", "--osc=no", "--start=90"])).unwrap();
        // Le fichier de liste est chargé à part : l'adresse du film n'est dans aucune option.
        assert_eq!(plan.playlist.as_deref(), Some("/tmp/liste.m3u"));
        let get = |name: &str| plan.options.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str());
        // L'image va dans la zone de Maku ; le canal JSON est celui du suivi de la lecture.
        assert_eq!(get("vo"), Some("libmpv"));
        assert_eq!(get("input-ipc-server"), Some("/tmp/s.sock"));
        // La liste arrive après le démarrage : mpv l'attend (« once »), puis s'arrête à la fin comme le programme.
        assert_eq!(get("idle"), Some("once"));
        // Une option sans valeur est activée ; une valeur garde ses « = ».
        assert_eq!(get("quiet"), Some("yes"));
        assert_eq!(get("title"), Some("Un film = deux"));
        assert_eq!(get("osc"), Some("no"));
        assert_eq!(get("start"), Some("90"));
        // Rien de ce qui règle la fenêtre de mpv : il n'y en a plus.
        for gone in ["fullscreen", "geometry", "force-window", "playlist"] {
            assert_eq!(get(gone), None, "{gone}");
        }
    }

    #[test]
    fn argument_qui_n_est_pas_une_option_refuse() {
        // Une adresse de film passée telle quelle, une autre sortie vidéo ou une fenêtre d'accueil : jamais.
        for bad in [&["http://serveur/film.mkv?jeton=SECRET"][..], &["--"], &["--idle=no", "film.mkv"], &["--vo=gpu"], &["--wid=42"]] {
            assert!(plan(&args(bad)).is_err(), "{bad:?}");
        }
        // Sans fichier de liste, le plan est rendu sans rien à lire : la lecture le refuse.
        assert_eq!(plan(&args(&["--idle=no"])).unwrap().playlist, None);
    }

    #[test]
    fn libmpv_l_emporte_sur_le_reglage_experimental() {
        // (dans la fenêtre par libmpv, mpv en plein écran sous la fenêtre)
        assert_eq!(modes(true, true), (true, false));
        assert_eq!(modes(true, false), (true, false));
        // Sans libmpv, le réglage expérimental garde son effet ; sans rien, mpv a sa fenêtre.
        assert_eq!(modes(false, true), (false, true));
        assert_eq!(modes(false, false), (false, false));
    }

    #[test]
    fn film_dans_la_fenetre_selon_l_environnement() {
        let env = |vars: &'static [(&'static str, &'static str)]| move |name: &str| vars.iter().find(|(n, _)| *n == name).map(|(_, v)| v.to_string());
        assert!(allowed(env(&[])));
        // Désactivée (« 0 » ou vide), une variable ne change rien.
        assert!(allowed(env(&[("MAKU_NO_LIBMPV", "0"), ("WEBKIT_DISABLE_DMABUF_RENDERER", ""), ("WEBKIT_DISABLE_COMPOSITING_MODE", " 0 ")])));
        // Refus de l'utilisateur, ou rendu de repli de la page (elle ne s'afficherait pas sur l'image).
        assert!(!allowed(env(&[("MAKU_NO_LIBMPV", "1")])));
        assert!(!allowed(env(&[("WEBKIT_DISABLE_DMABUF_RENDERER", "1")])));
        assert!(!allowed(env(&[("WEBKIT_DISABLE_COMPOSITING_MODE", "1")])));
    }
}
