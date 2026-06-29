using System.Net;
using System.Text;
using SwitchifyPc.Core.Updates;

namespace SwitchifyPc.Tests;

public sealed class GitHubReleaseUpdateBackendTests : IDisposable
{
    private readonly string tempDir = Path.Combine(Path.GetTempPath(), $"switchify-github-updates-{Guid.NewGuid():N}");

    [Fact]
    public async Task ReportsAvailableReleaseWithMatchingInstallerAsset()
    {
        FakeHttpHandler handler = new();
        handler.EnqueueJson("""
        [
          {
            "tag_name": "v0.2.0",
            "name": "Switchify PC 0.2.0",
            "body": "Native app.",
            "draft": false,
            "prerelease": false,
            "assets": [
              {
                "name": "Switchify-PC-Setup-0.2.0-x64.exe",
                "browser_download_url": "https://example.test/Switchify-PC-Setup-0.2.0-x64.exe"
              }
            ]
          }
        ]
        """);
        GitHubReleaseUpdateBackend backend = CreateBackend(handler);

        UpdateCheckOutcome outcome = await backend.CheckForUpdatesAsync("0.1.20");

        Assert.True(outcome.UpdateAvailable);
        Assert.NotNull(outcome.Update);
        Assert.Equal("0.2.0", outcome.Update.Version);
        Assert.Equal("Switchify PC 0.2.0", outcome.Update.ReleaseName);
        Assert.Equal("Native app.", outcome.Update.ReleaseNotes);
        Assert.Equal("Switchify-PC-Setup-0.2.0-x64.exe", outcome.Update.InstallerAssetName);
        Assert.Equal("https://example.test/Switchify-PC-Setup-0.2.0-x64.exe", outcome.Update.InstallerDownloadUrl);
        Assert.Equal("https://api.github.com/repos/switchifyapp/switchify-pc/releases", handler.Requests.Single().RequestUri?.ToString());
    }

    [Fact]
    public async Task IgnoresDraftsPrereleasesAndMissingInstallerAssets()
    {
        FakeHttpHandler handler = new();
        handler.EnqueueJson("""
        [
          {
            "tag_name": "v0.3.0",
            "name": "Draft",
            "body": "",
            "draft": true,
            "prerelease": false,
            "assets": [{ "name": "Switchify-PC-Setup-0.3.0-x64.exe", "browser_download_url": "https://example.test/draft.exe" }]
          },
          {
            "tag_name": "v0.2.1-beta.1",
            "name": "Beta",
            "body": "",
            "draft": false,
            "prerelease": true,
            "assets": [{ "name": "Switchify-PC-Setup-0.2.1-beta.1-x64.exe", "browser_download_url": "https://example.test/beta.exe" }]
          },
          {
            "tag_name": "v0.2.0",
            "name": "Missing asset",
            "body": "",
            "draft": false,
            "prerelease": false,
            "assets": [{ "name": "notes.txt", "browser_download_url": "https://example.test/notes.txt" }]
          }
        ]
        """);
        GitHubReleaseUpdateBackend backend = CreateBackend(handler);

        UpdateCheckOutcome outcome = await backend.CheckForUpdatesAsync("0.1.20");

        Assert.False(outcome.UpdateAvailable);
        Assert.Null(outcome.Update);
        Assert.Null(outcome.FailureReason);
    }

    [Fact]
    public async Task AllowsPrereleaseWhenCurrentVersionIsPrerelease()
    {
        FakeHttpHandler handler = new();
        handler.EnqueueJson("""
        [
          {
            "tag_name": "v0.2.1-beta.2",
            "name": "Beta",
            "body": "",
            "draft": false,
            "prerelease": true,
            "assets": [{ "name": "Switchify-PC-Setup-0.2.1-beta.2-x64.exe", "browser_download_url": "https://example.test/beta.exe" }]
          }
        ]
        """);
        GitHubReleaseUpdateBackend backend = CreateBackend(handler);

        UpdateCheckOutcome outcome = await backend.CheckForUpdatesAsync("0.2.1-beta.1");

        Assert.True(outcome.UpdateAvailable);
        Assert.Equal("0.2.1-beta.2", outcome.Update?.Version);
    }

    [Fact]
    public async Task ReportsNetworkFailureForUnsuccessfulReleaseResponse()
    {
        FakeHttpHandler handler = new();
        handler.Enqueue(new HttpResponseMessage(HttpStatusCode.InternalServerError));
        GitHubReleaseUpdateBackend backend = CreateBackend(handler);

        UpdateCheckOutcome outcome = await backend.CheckForUpdatesAsync("0.1.20");

        Assert.False(outcome.UpdateAvailable);
        Assert.Equal(UpdateFailureReason.NetworkError, outcome.FailureReason);
    }

    [Fact]
    public async Task DownloadsInstallerAssetToCacheAndReportsProgress()
    {
        FakeHttpHandler handler = new();
        byte[] installerBytes = Encoding.UTF8.GetBytes("installer");
        handler.Enqueue(new HttpResponseMessage(HttpStatusCode.OK)
        {
            Content = new ByteArrayContent(installerBytes)
            {
                Headers = { ContentLength = installerBytes.Length }
            }
        });
        GitHubReleaseUpdateBackend backend = CreateBackend(handler);
        List<UpdateDownloadSnapshot> progress = [];

        UpdateDownloadOutcome outcome = await backend.DownloadUpdateAsync(
            new AvailableUpdate(
                Version: "0.2.0",
                ReleaseName: null,
                ReleaseNotes: null,
                InstallerAssetName: "Switchify-PC-Setup-0.2.0-x64.exe",
                InstallerDownloadUrl: "https://example.test/Switchify-PC-Setup-0.2.0-x64.exe"),
            new ProgressCollector(progress));

        Assert.Null(outcome.FailureReason);
        Assert.NotNull(outcome.InstallerPath);
        Assert.Equal(installerBytes, File.ReadAllBytes(outcome.InstallerPath));
        Assert.Equal(100, progress.Last().Percent);
        Assert.Equal("https://example.test/Switchify-PC-Setup-0.2.0-x64.exe", handler.Requests.Single().RequestUri?.ToString());
        Assert.DoesNotContain(Directory.EnumerateFiles(tempDir), path => path.EndsWith(".tmp", StringComparison.OrdinalIgnoreCase));
    }

    [Fact]
    public async Task DownloadFailsWhenInstallerMetadataIsMissing()
    {
        GitHubReleaseUpdateBackend backend = CreateBackend(new FakeHttpHandler());

        UpdateDownloadOutcome outcome = await backend.DownloadUpdateAsync(
            new AvailableUpdate("0.2.0", null, null),
            new ProgressCollector([]));

        Assert.Equal(UpdateFailureReason.InvalidUpdate, outcome.FailureReason);
    }

    public void Dispose()
    {
        if (Directory.Exists(tempDir))
        {
            Directory.Delete(tempDir, recursive: true);
        }
    }

    private GitHubReleaseUpdateBackend CreateBackend(FakeHttpHandler handler)
    {
        return new GitHubReleaseUpdateBackend(
            new HttpClient(handler),
            owner: "switchifyapp",
            repository: "switchify-pc",
            cacheDirectory: tempDir);
    }

    private sealed class FakeHttpHandler : HttpMessageHandler
    {
        private readonly Queue<HttpResponseMessage> responses = new();
        public List<HttpRequestMessage> Requests { get; } = [];

        public void EnqueueJson(string json)
        {
            Enqueue(new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent(json, Encoding.UTF8, "application/json")
            });
        }

        public void Enqueue(HttpResponseMessage response)
        {
            responses.Enqueue(response);
        }

        protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken)
        {
            Requests.Add(request);
            if (responses.Count == 0)
            {
                throw new InvalidOperationException("No fake HTTP response was queued.");
            }

            return Task.FromResult(responses.Dequeue());
        }
    }

    private sealed class ProgressCollector(List<UpdateDownloadSnapshot> snapshots) : IProgress<UpdateDownloadSnapshot>
    {
        public void Report(UpdateDownloadSnapshot value)
        {
            snapshots.Add(value);
        }
    }
}
