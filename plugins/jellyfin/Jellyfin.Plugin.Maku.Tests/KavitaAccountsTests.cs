using System;
using System.Collections.Generic;
using System.Linq;
using System.Net;
using System.Net.Http;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;
using Jellyfin.Plugin.Maku.Configuration;
using Jellyfin.Plugin.Maku.Relay;
using Xunit;

namespace Jellyfin.Plugin.Maku.Tests;

/// <summary>
/// Kavita par le plugin : création du compte Kavita d'un utilisateur Jellyfin, ses bibliothèques, et les clés qui
/// ouvrent les images. Une erreur ici ferait lire quelqu'un avec le compte d'un autre, ou laisserait passer n'importe qui.
/// </summary>
public class KavitaAccountsTests
{
    private const string Alice = "0123456789abcdef0123456789abcdef";
    private const string Bruno = "fedcba9876543210fedcba9876543210";

    /// <summary>Faux Kavita : clé administrateur « cle-admin » ; le compte créé reçoit « cle-alice » et « image-alice ».</summary>
    private sealed class FakeKavita : HttpMessageHandler
    {
        public List<(string Method, string Path, string? Key, string? Auth, JsonElement Body)> Calls { get; } = [];

        /// <summary>Gets or sets a value indicating whether l'invitation échoue (compte déjà invité, création interrompue).</summary>
        public bool AlreadyInvited { get; set; }

        protected override async Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken)
        {
            var key = request.Headers.TryGetValues("x-api-key", out var k) ? string.Join(",", k) : null;
            var auth = request.Headers.Authorization?.ToString();
            var text = request.Content is null ? "null" : await request.Content.ReadAsStringAsync(cancellationToken);
            var body = JsonDocument.Parse(text).RootElement.Clone();
            var path = request.RequestUri!.PathAndQuery;
            Calls.Add((request.Method.Method, path, key, auth, body));
            var admin = key == "cle-admin";
            (HttpStatusCode Status, string Json) reply = path switch
            {
                "/api/Library/libraries" when admin => (HttpStatusCode.OK, """[{"id":1,"name":"Mangas","type":0},{"id":2,"name":"BD","type":1}]"""),
                "/api/Account/invite" when admin && AlreadyInvited => (HttpStatusCode.BadRequest, "\"user-already-invited\""),
                "/api/Account/invite" when admin => (HttpStatusCode.OK, "{\"emailLink\":\"http://kavita.interne:5000/registration/confirm-email?token=jeton%2Bsp%C3%A9cial&email=x\",\"emailSent\":false}"),
                "/api/Users?includePending=true" when admin => (HttpStatusCode.OK, $$"""[{"id":3,"email":"autre@x"},{"id":7,"email":"MAKU-JELLYFIN-{{Alice}}@maku.invalid"}]"""),
                "/api/Account/invite-url?userId=7&withBaseUrl=true" when admin => (HttpStatusCode.OK, "http://kavita.interne:5000/registration/confirm-email?token=jeton%2Bsp%C3%A9cial&email=x"),
                "/api/Account/confirm-email" when key is null && body.GetProperty("token").GetString() == "jeton+spécial" => (HttpStatusCode.OK, """{"id":7,"token":"jwt-7"}"""),
                "/api/Account/create-auth-key" when auth == "Bearer jwt-7" => (HttpStatusCode.OK, """{"id":3,"key":"cle-alice","name":"Maku"}"""),
                "/api/Account/update" when admin => (HttpStatusCode.OK, string.Empty),
                "/api/Account/auth-keys" when key == "cle-alice" => (HttpStatusCode.OK, """[{"key":"cle-alice"},{"key":"image-alice"}]"""),
                _ => (HttpStatusCode.Unauthorized, string.Empty),
            };
            return new HttpResponseMessage(reply.Status) { Content = new StringContent(reply.Json) };
        }
    }

    private static PluginConfiguration Config() => new() { KavitaUrl = "http://kavita.invalid:5000/", KavitaApiKey = "cle-admin" };

    [Fact]
    public async Task Compte_cree_invitation_activation_puis_cle_pour_Maku()
    {
        var kavita = new FakeKavita();
        using var http = new HttpClient(kavita);
        var account = await KavitaAccounts.Create(http, Config(), Alice, "Élodie Lectrice", [2], CancellationToken.None);
        Assert.Equal(("Elodie-Lectrice.abcdef", 7, "cle-alice"), (account.Username, account.KavitaId, account.Key));
        Assert.Equal([2], account.Libraries);
        // Invitation avec la clé administrateur, sans limite d'âge, rôle de connexion, la bonne bibliothèque.
        var invite = kavita.Calls.Single(c => c.Path == "/api/Account/invite");
        Assert.Equal("cle-admin", invite.Key);
        Assert.Equal($"maku-jellyfin-{Alice}@maku.invalid", invite.Body.GetProperty("email").GetString());
        Assert.Equal("Login", invite.Body.GetProperty("roles")[0].GetString());
        Assert.Equal(2, invite.Body.GetProperty("libraries")[0].GetInt32());
        Assert.Equal(-1, invite.Body.GetProperty("ageRestriction").GetProperty("ageRating").GetInt32());
        // Activation sans clé, mot de passe tiré au hasard ; puis la clé de Maku, avec le jeton du nouveau compte.
        var confirm = kavita.Calls.Single(c => c.Path == "/api/Account/confirm-email");
        Assert.Null(confirm.Key);
        Assert.Equal(48, confirm.Body.GetProperty("password").GetString()!.Length);
        Assert.Equal("Bearer jwt-7", kavita.Calls.Single(c => c.Path == "/api/Account/create-auth-key").Auth);
    }

    [Fact]
    public async Task Creation_interrompue_reprise_par_le_lien_d_activation()
    {
        var kavita = new FakeKavita { AlreadyInvited = true };
        using var http = new HttpClient(kavita);
        var account = await KavitaAccounts.Create(http, Config(), Alice, "Alice", [1], CancellationToken.None);
        Assert.Equal("cle-alice", account.Key);
        Assert.Contains(kavita.Calls, c => c.Path == "/api/Account/invite-url?userId=7&withBaseUrl=true");
    }

    [Fact]
    public async Task Bibliotheques_changees_avec_la_cle_administrateur()
    {
        var kavita = new FakeKavita();
        using var http = new HttpClient(kavita);
        var account = new KavitaAccount { JellyfinUserId = Alice, KavitaId = 7, Username = "Alice.abcdef", Key = "cle-alice" };
        await KavitaAccounts.SetLibraries(http, Config(), account, [], CancellationToken.None);
        var update = kavita.Calls.Single(c => c.Path == "/api/Account/update");
        Assert.Equal("cle-admin", update.Key);
        Assert.Equal(7, update.Body.GetProperty("userId").GetInt32());
        Assert.Equal(0, update.Body.GetProperty("libraries").GetArrayLength());
        Assert.Equal("Login", update.Body.GetProperty("roles")[0].GetString());
        var libraries = await KavitaAccounts.Libraries(http, Config(), CancellationToken.None);
        Assert.Equal([new KavitaLibrary(1, "Mangas", 0), new KavitaLibrary(2, "BD", 1)], libraries);
    }

    [Fact]
    public async Task Images_ouvertes_aux_seules_cles_vues_depuis_moins_d_un_jour()
    {
        var kavita = new FakeKavita();
        using var http = new HttpClient(kavita);
        using var accounts = new KavitaAccounts();
        var now = DateTime.UtcNow;
        Assert.False(accounts.Fresh("image-alice", now));
        // La clé d'un utilisateur reconnu, et ses voisines (celle des images d'un livre).
        var keys = await KavitaAccounts.KeysOf(http, "http://kavita.invalid:5000", "cle-alice", CancellationToken.None);
        Assert.Equal(["cle-alice", "cle-alice", "image-alice"], keys);
        accounts.MarkSeen(keys, now);
        Assert.True(accounts.Fresh("image-alice", now.AddHours(23)));
        Assert.False(accounts.Fresh("image-alice", now.AddHours(25)));
        Assert.False(accounts.Fresh("autre", now));
        Assert.False(accounts.Fresh(string.Empty, now));
    }

    [Fact]
    public void Cle_de_chacun_selon_ses_bibliotheques()
    {
        var c = Config();
        c.KavitaAccounts.Add(new KavitaAccount { JellyfinUserId = Alice, Key = "cle-alice", Libraries = [2] });
        c.KavitaAccounts.Add(new KavitaAccount { JellyfinUserId = Bruno, Key = "cle-bruno", Libraries = [] });
        // L'administrateur du serveur lit avec le compte administrateur de Kavita.
        Assert.Equal("cle-admin", KavitaAccounts.KeyFor(c, Alice, admin: true));
        // Un utilisateur : la clé de son compte, quelle que soit la façon d'écrire son identifiant.
        Assert.Equal("cle-alice", KavitaAccounts.KeyFor(c, "01234567-89AB-CDEF-0123-456789ABCDEF", admin: false));
        // Plus aucune bibliothèque, ou pas de compte : rien.
        Assert.Null(KavitaAccounts.KeyFor(c, Bruno, admin: false));
        Assert.Null(KavitaAccounts.KeyFor(c, "ffffffffffffffffffffffffffffffff", admin: false));
        Assert.Null(KavitaAccounts.KeyFor(c, string.Empty, admin: false));
    }

    [Theory]
    [InlineData("jellyfin:" + Alice, Bruno, Alice)]
    [InlineData("jellyfin:01234567-89ab-cdef-0123-456789abcdef", Bruno, Alice)]
    [InlineData("jellyfin:" + Alice, Alice, null)]
    [InlineData("plex:42", Bruno, null)]
    [InlineData("jellyfin:../x", Bruno, null)]
    [InlineData("jellyfin:0123456789abcdef0123456789abcdeg", Bruno, null)]
    [InlineData("jellyfin:", Bruno, null)]
    public void Qui_peut_recevoir_des_bibliotheques(string id, string me, string? expected)
        => Assert.Equal(expected, KavitaAccounts.GrantTarget(id, me));

    [Fact]
    public void Sans_jeton_Jellyfin_seule_une_cle_vue_passe()
    {
        using var accounts = new KavitaAccounts();
        var now = DateTime.UtcNow;
        accounts.MarkSeen(["image-alice"], now);
        // Un utilisateur reconnu par Jellyfin passe (Kavita vérifie sa clé).
        Assert.True(accounts.MayPass(true, string.Empty, now));
        // Sans jeton : seulement une clé vue récemment.
        Assert.True(accounts.MayPass(false, "image-alice", now));
        Assert.False(accounts.MayPass(false, "cle-inconnue", now));
        Assert.False(accounts.MayPass(false, string.Empty, now));
        Assert.False(accounts.MayPass(false, "image-alice", now.AddDays(2)));
    }

    [Theory]
    [InlineData("Élodie Lectrice", Alice, "Elodie-Lectrice.abcdef")]
    [InlineData("  ", Alice, "lecteur.abcdef")]
    [InlineData("李 明", Alice, "lecteur.abcdef")]
    public void Nom_du_compte_Kavita(string name, string id, string expected)
        => Assert.Equal(expected, KavitaAccounts.UsernameFor(name, id));

    [Theory]
    [InlineData("api/Series/5", "api/Series/5")]
    [InlineData("/api/image/series-cover", "api/image/series-cover")]
    [InlineData("registration/confirm-email", null)]
    [InlineData("api/../registration", null)]
    [InlineData("api/%2e%2e/x", null)]
    [InlineData("api%2fx", null)]
    [InlineData("", null)]
    public void Seulement_l_API_de_Kavita(string path, string? expected)
        => Assert.Equal(expected, KavitaAccounts.ApiPath(path));
}
