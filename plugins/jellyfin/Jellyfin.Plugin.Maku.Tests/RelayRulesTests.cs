using System.Text.Json;
using Jellyfin.Plugin.Maku.Relay;
using Xunit;

namespace Jellyfin.Plugin.Maku.Tests;

public class RelayRulesTests
{
    [Theory]
    [InlineData("api/v1/request", "api/v1/request")]
    [InlineData("/api/v1/auth/me", "api/v1/auth/me")]
    [InlineData("api/v1", "api/v1")]
    [InlineData("API/V1/status", "API/V1/status")]
    public void Chemins_de_l_API_acceptes(string path, string expected)
        => Assert.Equal(expected, RelayRules.ApiPath(path));

    [Theory]
    [InlineData("")]
    [InlineData(null)]
    [InlineData("login")]
    [InlineData("api/v2/request")]
    [InlineData("api/v1x")]
    [InlineData("api/v1/../settings")]
    [InlineData("api/v1/%2e%2e/settings")]
    [InlineData("api/v1/a%2fb")]
    [InlineData(@"api/v1/a\b")]
    public void Chemins_hors_de_l_API_refuses(string? path)
        => Assert.Null(RelayRules.ApiPath(path));

    [Theory]
    [InlineData("Authorization")]
    [InlineData("X-Emby-Token")]
    [InlineData("x-emby-authorization")]
    [InlineData("X-Maku-Token")]
    [InlineData("X-Api-Key")]
    [InlineData("X-API-User")]
    [InlineData("Host")]
    [InlineData("Content-Length")]
    public void Jeton_cle_et_utilisateur_jamais_transmis(string header)
    {
        Assert.False(RelayRules.Forwarded(header, keepCookies: true));
        Assert.False(RelayRules.Returned(header));
    }

    [Fact]
    public void Cookies_transmis_a_Tracearr_seulement()
    {
        Assert.True(RelayRules.Forwarded("Cookie", keepCookies: true));
        Assert.False(RelayRules.Forwarded("Cookie", keepCookies: false));
        Assert.True(RelayRules.Forwarded("Accept-Language", keepCookies: false));
        Assert.False(RelayRules.Returned("Set-Cookie"));
    }

    [Fact]
    public void Domaine_retire_des_cookies()
        => Assert.Equal("sid=1; Path=/; HttpOnly", RelayRules.StripDomain("sid=1; Domain=tracearr.lan; Path=/; HttpOnly"));

    [Theory]
    [InlineData("http://192.168.1.10:5055/", "http://192.168.1.10:5055")]
    [InlineData(" https://seerr.lan ", "https://seerr.lan")]
    [InlineData("seerr.lan", "")]
    [InlineData("ftp://seerr.lan", "")]
    [InlineData("", "")]
    public void Adresses_nettoyees(string url, string expected)
        => Assert.Equal(expected, RelayRules.CleanBase(url));

    [Fact]
    public void Compte_Seerr_retrouve_par_l_identifiant_Jellyfin()
    {
        var json = JsonDocument.Parse("""
            {"pageInfo":{"results":3},"results":[
              {"id":1,"displayName":"Admin","plexUsername":"admin"},
              {"id":7,"displayName":"Camille","jellyfinUserId":"0F1E2D3C4B5A69788796A5B4C3D2E1F0"},
              {"id":9,"username":"hugo","jellyfinUserId":""}
            ]}
            """);
        var accounts = SeerrAccounts.Parse(json.RootElement);
        Assert.Equal(3, accounts.Count);
        Assert.Equal("Camille", accounts[1].Name);
        Assert.Equal("hugo", accounts[2].Name);
        Assert.Null(accounts[2].JellyfinUserId);
        var id = RelayRules.NormalizeUserId("0f1e2d3c-4b5a-6978-8796-a5b4c3d2e1f0");
        Assert.Equal(7, SeerrAccounts.Find(accounts, id));
        Assert.Null(SeerrAccounts.Find(accounts, RelayRules.NormalizeUserId("11111111-2222-3333-4444-555555555555")));
    }
}
