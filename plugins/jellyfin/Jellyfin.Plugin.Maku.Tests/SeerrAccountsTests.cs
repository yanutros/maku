using System.Collections.Generic;
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
/// Sous quel compte Seerr agit un utilisateur Jellyfin : une erreur ici ferait faire ses demandes à quelqu'un sous le
/// compte d'un autre.
/// </summary>
public class SeerrAccountsTests
{
    private const string Alice = "0123456789abcdef0123456789abcdef";
    private const string Bruno = "fedcba9876543210fedcba9876543210";

    /// <summary>Faux Seerr : rend la liste de ses comptes, et compte les demandes (avec la clé d'API reçue).</summary>
    private sealed class FakeSeerr : HttpMessageHandler
    {
        public int Calls { get; private set; }

        public string? Key { get; private set; }

        public HttpStatusCode Status { get; set; } = HttpStatusCode.OK;

        public string Body { get; set; } = """
            {"results":[
              {"id":1,"displayName":"Admin"},
              {"id":7,"displayName":"Alice","jellyfinUserId":"0123456789ABCDEF0123456789ABCDEF"},
              {"id":9,"username":"bruno","jellyfinUserId":"fedcba98-7654-3210-fedc-ba9876543210"}
            ]}
            """;

        protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken)
        {
            Calls++;
            Key = request.Headers.TryGetValues("X-Api-Key", out var values) ? string.Join(",", values) : null;
            return Task.FromResult(new HttpResponseMessage(Status) { Content = new StringContent(Body) });
        }
    }

    private static PluginConfiguration Config(params UserLink[] links)
        => new() { SeerrUrl = "http://seerr.invalid:5055/", SeerrApiKey = "cle", UserLinks = new List<UserLink>(links) };

    [Fact]
    public async Task Compte_importe_de_Jellyfin_retrouve_par_son_identifiant()
    {
        var seerr = new FakeSeerr();
        using var http = new HttpClient(seerr);
        using var accounts = new SeerrAccounts();
        // L'identifiant s'écrit avec ou sans tirets, en majuscules ou non : c'est le même utilisateur.
        Assert.Equal(7, await accounts.AccountFor(http, Config(), Alice, CancellationToken.None));
        Assert.Equal(9, await accounts.AccountFor(http, Config(), Bruno, CancellationToken.None));
        Assert.Equal(9, await accounts.AccountFor(http, Config(), "FEDCBA98-7654-3210-FEDC-BA9876543210", CancellationToken.None));
        // La liste des comptes n'est lue qu'une fois, avec la clé d'API du plugin.
        Assert.Equal(1, seerr.Calls);
        Assert.Equal("cle", seerr.Key);
    }

    [Fact]
    public async Task Utilisateur_sans_compte_Seerr_n_agit_sous_aucun_compte()
    {
        var seerr = new FakeSeerr();
        using var http = new HttpClient(seerr);
        using var accounts = new SeerrAccounts();
        // Ni le compte de l'administrateur de la clé (1), ni celui d'un autre.
        Assert.Null(await accounts.AccountFor(http, Config(), "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", CancellationToken.None));
        Assert.Null(await accounts.AccountFor(http, Config(), string.Empty, CancellationToken.None));
    }

    [Fact]
    public async Task Lien_choisi_par_l_administrateur_prioritaire()
    {
        var seerr = new FakeSeerr();
        using var http = new HttpClient(seerr);
        using var accounts = new SeerrAccounts();
        var config = Config(new UserLink { JellyfinUserId = "01234567-89AB-CDEF-0123-456789ABCDEF", SeerrUserId = 42 });
        // Alice a un compte importé (7), mais l'administrateur l'a liée au compte 42 : sans même lire Seerr.
        Assert.Equal(42, await accounts.AccountFor(http, config, Alice, CancellationToken.None));
        Assert.Equal(0, seerr.Calls);
        // Le lien d'Alice ne vaut pas pour Bruno.
        Assert.Equal(9, await accounts.AccountFor(http, config, Bruno, CancellationToken.None));
        // Un lien sans compte (0) ne compte pas : on retombe sur le compte importé.
        var empty = Config(new UserLink { JellyfinUserId = Alice, SeerrUserId = 0 });
        Assert.Equal(7, await accounts.AccountFor(http, empty, Alice, CancellationToken.None));
    }

    [Fact]
    public async Task Cle_d_API_refusee_signalee_et_non_prise_pour_une_absence_de_compte()
    {
        var seerr = new FakeSeerr { Status = HttpStatusCode.Forbidden };
        using var http = new HttpClient(seerr);
        using var accounts = new SeerrAccounts();
        await Assert.ThrowsAsync<HttpRequestException>(() => accounts.AccountFor(http, Config(), Alice, CancellationToken.None));
    }

    [Fact]
    public async Task Autre_Seerr_ou_autre_cle_relit_les_comptes()
    {
        var seerr = new FakeSeerr();
        using var http = new HttpClient(seerr);
        using var accounts = new SeerrAccounts();
        Assert.Equal(7, await accounts.AccountFor(http, Config(), Alice, CancellationToken.None));
        // L'administrateur change la clé : les comptes gardés étaient ceux de l'ancienne.
        var other = Config();
        other.SeerrApiKey = "autre-cle";
        seerr.Body = """{"results":[{"id":3,"displayName":"Alice","jellyfinUserId":"0123456789abcdef0123456789abcdef"}]}""";
        Assert.Equal(3, await accounts.AccountFor(http, other, Alice, CancellationToken.None));
        Assert.Equal(2, seerr.Calls);
        Assert.Equal("autre-cle", seerr.Key);
    }

    [Fact]
    public void Comptes_lus_dans_la_reponse_de_Seerr()
    {
        using var doc = JsonDocument.Parse("""{"results":[{"id":5,"email":"a@exemple.invalid"},{"displayName":"sans numéro"},{"id":"x"},{"id":6,"jellyfinUserId":""}]}""");
        var list = SeerrAccounts.Parse(doc.RootElement);
        Assert.Equal(2, list.Count);
        Assert.Equal((5, "a@exemple.invalid", null), (list[0].Id, list[0].Name, list[0].JellyfinUserId));
        // Un identifiant Jellyfin vide n'en est pas un : il ne doit correspondre à personne.
        Assert.Equal((6, "#6", null), (list[1].Id, list[1].Name, list[1].JellyfinUserId));
        using var empty = JsonDocument.Parse("""{"message":"Unauthorized"}""");
        Assert.Empty(SeerrAccounts.Parse(empty.RootElement));
    }
}
