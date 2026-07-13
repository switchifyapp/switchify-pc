using System.Net;
using System.Text.Json;
using SwitchifyPc.Core.Diagnostics;

namespace SwitchifyPc.Tests;

public sealed class TelemetryTests
{
    [Fact]
    public void SettingsDefaultToOptedOutWithStableOpaqueInstallId()
    {
        using TemporaryDirectory directory = new();
        JsonTelemetrySettingsStore store = new(Path.Combine(directory.Path, "settings.json"));

        TelemetrySettings first = store.Load();
        TelemetrySettings second = store.Load();

        Assert.False(first.Enabled);
        Assert.False(first.ConsentRecorded);
        Assert.True(Guid.TryParse(first.InstallId, out _));
        Assert.Equal(first.InstallId, second.InstallId);
    }

    [Fact]
    public void SanitizerRemovesSecretsAndUserProfilePaths()
    {
        string input = "Authorization: Bearer abc123 token=secret pairing_code=654321 tb_live_value at C:\\Users\\Alice\\private.txt";

        string safe = TelemetrySanitizer.Message(input)!;

        Assert.DoesNotContain("abc123", safe);
        Assert.DoesNotContain("secret", safe);
        Assert.DoesNotContain("654321", safe);
        Assert.DoesNotContain("tb_live_value", safe);
        Assert.DoesNotContain("Alice", safe);
        Assert.Contains("[redacted]", safe);
    }

    [Fact]
    public async Task OptedOutReporterDoesNotSendOrQueue()
    {
        using TemporaryDirectory directory = new();
        JsonTelemetrySettingsStore store = new(Path.Combine(directory.Path, "settings.json"));
        RecordingHandler handler = new(HttpStatusCode.OK);
        using TelemetryReporter reporter = CreateReporter(directory, store, handler);

        await reporter.ReportExceptionAsync(new InvalidOperationException("failure"), "test");

        Assert.Empty(handler.Requests);
        Assert.False(File.Exists(Path.Combine(directory.Path, "queue.json")));
    }

    [Fact]
    public async Task ExceptionIsSanitizedAuthenticatedAndRemovedAfterSuccessfulUpload()
    {
        using TemporaryDirectory directory = new();
        JsonTelemetrySettingsStore store = new(Path.Combine(directory.Path, "settings.json"));
        store.Save(true);
        RecordingHandler handler = new(HttpStatusCode.OK);
        using TelemetryReporter reporter = CreateReporter(directory, store, handler);

        await reporter.ReportExceptionAsync(new InvalidOperationException("token=private-value"), "runtime");

        RecordingRequest request = Assert.Single(handler.Requests);
        Assert.Equal("Bearer", request.Scheme);
        Assert.Equal("test-key", request.Parameter);
        Assert.DoesNotContain("private-value", request.Body);
        Assert.Contains("switchify-pc", request.Body);
        Assert.Contains(store.Load().InstallId, request.Body);
        Assert.False(File.Exists(Path.Combine(directory.Path, "queue.json")));
    }

    [Fact]
    public async Task FailedExceptionUploadQueuesAndOptOutPurges()
    {
        using TemporaryDirectory directory = new();
        JsonTelemetrySettingsStore store = new(Path.Combine(directory.Path, "settings.json"));
        store.Save(true);
        RecordingHandler handler = new(HttpStatusCode.ServiceUnavailable);
        string queuePath = Path.Combine(directory.Path, "queue.json");
        using TelemetryReporter reporter = CreateReporter(directory, store, handler);

        await reporter.ReportExceptionAsync(new Exception("failure"), "runtime");
        Assert.True(File.Exists(queuePath));

        reporter.SetEnabled(false);

        Assert.False(File.Exists(queuePath));
        Assert.False(store.Load().Enabled);
        Assert.True(store.Load().ConsentRecorded);
    }

    [Fact]
    public async Task HealthFailuresAreNotQueued()
    {
        using TemporaryDirectory directory = new();
        JsonTelemetrySettingsStore store = new(Path.Combine(directory.Path, "settings.json"));
        store.Save(true);
        RecordingHandler handler = new(HttpStatusCode.ServiceUnavailable);
        using TelemetryReporter reporter = CreateReporter(directory, store, handler);

        await reporter.ReportHealthAsync("app.startup.completed");

        Assert.Single(handler.Requests);
        Assert.False(File.Exists(Path.Combine(directory.Path, "queue.json")));
    }

    private static TelemetryReporter CreateReporter(TemporaryDirectory directory, JsonTelemetrySettingsStore store, RecordingHandler handler)
    {
        return new TelemetryReporter(store, new HttpClient(handler), "test-key", "0.6.4", Path.Combine(directory.Path, "queue.json"));
    }

    private sealed record RecordingRequest(string? Scheme, string? Parameter, string Body);

    private sealed class RecordingHandler(HttpStatusCode statusCode) : HttpMessageHandler
    {
        public List<RecordingRequest> Requests { get; } = [];

        protected override async Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken)
        {
            Requests.Add(new(
                request.Headers.Authorization?.Scheme,
                request.Headers.Authorization?.Parameter,
                request.Content is null ? string.Empty : await request.Content.ReadAsStringAsync(cancellationToken)));
            return new HttpResponseMessage(statusCode);
        }
    }

    private sealed class TemporaryDirectory : IDisposable
    {
        public TemporaryDirectory()
        {
            Path = System.IO.Path.Combine(System.IO.Path.GetTempPath(), "switchify-telemetry-tests", Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(Path);
        }

        public string Path { get; }

        public void Dispose()
        {
            try { Directory.Delete(Path, true); } catch { }
        }
    }
}
