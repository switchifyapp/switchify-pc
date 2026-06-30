using System.Globalization;
using System.Net.Http.Headers;
using System.Text.Json;
using System.Text.RegularExpressions;
using SwitchifyPc.Core.Storage;

namespace SwitchifyPc.Core.Updates;

public sealed class GitHubReleaseUpdateBackend : IUpdateBackend
{
    private const string InstallerAssetPrefix = "Switchify-PC-Setup-";
    private const string InstallerAssetSuffix = "-x64.exe";

    private readonly HttpClient httpClient;
    private readonly string owner;
    private readonly string repository;
    private readonly string cacheDirectory;

    public GitHubReleaseUpdateBackend(HttpClient httpClient, string owner, string repository, string cacheDirectory)
    {
        this.httpClient = httpClient;
        this.owner = owner;
        this.repository = repository;
        this.cacheDirectory = cacheDirectory;
        if (!this.httpClient.DefaultRequestHeaders.UserAgent.Any())
        {
            this.httpClient.DefaultRequestHeaders.UserAgent.Add(new ProductInfoHeaderValue("Switchify-PC", "0.2"));
        }
    }

    public async Task<UpdateCheckOutcome> CheckForUpdatesAsync(string currentVersion, CancellationToken cancellationToken = default)
    {
        try
        {
            bool includePrerelease = SemVersion.Parse(currentVersion).IsPrerelease;
            using HttpResponseMessage response = await httpClient.GetAsync(
                $"https://api.github.com/repos/{owner}/{repository}/releases",
                cancellationToken).ConfigureAwait(false);

            if (!response.IsSuccessStatusCode)
            {
                return UpdateCheckOutcome.Failed(UpdateFailureReason.NetworkError);
            }

            await using Stream stream = await response.Content.ReadAsStreamAsync(cancellationToken).ConfigureAwait(false);
            IReadOnlyList<GitHubRelease> releases = await JsonSerializer.DeserializeAsync<List<GitHubRelease>>(
                stream,
                JsonOptions,
                cancellationToken).ConfigureAwait(false) ?? [];

            foreach (GitHubRelease release in releases)
            {
                if (release.Draft) continue;
                if (release.Prerelease && !includePrerelease) continue;
                string version = NormalizeVersion(release.TagName);
                if (!SemVersion.IsNewer(version, currentVersion)) continue;

                GitHubReleaseAsset? installer = release.Assets.FirstOrDefault(asset =>
                    string.Equals(asset.Name, InstallerAssetName(version), StringComparison.OrdinalIgnoreCase) &&
                    !string.IsNullOrWhiteSpace(asset.BrowserDownloadUrl));

                if (installer is null) continue;

                return UpdateCheckOutcome.Available(new AvailableUpdate(
                    Version: version,
                    ReleaseName: release.Name,
                    ReleaseNotes: release.Body,
                    InstallerAssetName: installer.Name,
                    InstallerDownloadUrl: installer.BrowserDownloadUrl));
            }

            return UpdateCheckOutcome.UpToDate();
        }
        catch (OperationCanceledException)
        {
            throw;
        }
        catch
        {
            return UpdateCheckOutcome.Failed(UpdateFailureReason.NetworkError);
        }
    }

    public async Task<UpdateDownloadOutcome> DownloadUpdateAsync(
        AvailableUpdate update,
        IProgress<UpdateDownloadSnapshot> progress,
        CancellationToken cancellationToken = default)
    {
        if (string.IsNullOrWhiteSpace(update.InstallerAssetName) || string.IsNullOrWhiteSpace(update.InstallerDownloadUrl))
        {
            return UpdateDownloadOutcome.Failed(UpdateFailureReason.InvalidUpdate);
        }

        try
        {
            using HttpResponseMessage response = await httpClient.GetAsync(
                update.InstallerDownloadUrl,
                HttpCompletionOption.ResponseHeadersRead,
                cancellationToken).ConfigureAwait(false);

            if (!response.IsSuccessStatusCode)
            {
                return UpdateDownloadOutcome.Failed(UpdateFailureReason.NetworkError);
            }

            Directory.CreateDirectory(cacheDirectory);
            string outputPath = Path.Combine(cacheDirectory, SafeInstallerAssetName(update.InstallerAssetName));
            string tempPath = $"{outputPath}.{Environment.ProcessId}.{DateTimeOffset.UtcNow.ToUnixTimeMilliseconds()}.{Guid.NewGuid():N}.tmp";
            long? totalBytes = response.Content.Headers.ContentLength;
            long downloadedBytes = 0;
            var renamed = false;

            try
            {
                {
                    await using Stream input = await response.Content.ReadAsStreamAsync(cancellationToken).ConfigureAwait(false);
                    await using FileStream output = new(tempPath, FileMode.Create, FileAccess.Write, FileShare.None, 81920, FileOptions.Asynchronous | FileOptions.WriteThrough);
                    byte[] buffer = new byte[81920];

                    while (true)
                    {
                        int read = await input.ReadAsync(buffer, cancellationToken).ConfigureAwait(false);
                        if (read == 0) break;

                        await output.WriteAsync(buffer.AsMemory(0, read), cancellationToken).ConfigureAwait(false);
                        downloadedBytes += read;
                        progress.Report(new UpdateDownloadSnapshot(downloadedBytes, totalBytes, Percent(downloadedBytes, totalBytes)));
                    }

                    await output.FlushAsync(cancellationToken).ConfigureAwait(false);
                    output.Flush(flushToDisk: true);
                }

                File.Move(tempPath, outputPath, overwrite: true);
                renamed = true;
            }
            finally
            {
                if (!renamed)
                {
                    TryDelete(tempPath);
                }
            }

            progress.Report(new UpdateDownloadSnapshot(downloadedBytes, totalBytes, 100));
            return UpdateDownloadOutcome.Downloaded(outputPath);
        }
        catch (OperationCanceledException)
        {
            throw;
        }
        catch
        {
            return UpdateDownloadOutcome.Failed(UpdateFailureReason.NetworkError);
        }
    }

    public static string InstallerAssetName(string version) => $"{InstallerAssetPrefix}{version}{InstallerAssetSuffix}";

    private static string NormalizeVersion(string tagName)
    {
        return tagName.StartsWith('v') || tagName.StartsWith('V') ? tagName[1..] : tagName;
    }

    private static int? Percent(long downloadedBytes, long? totalBytes)
    {
        if (totalBytes is null or <= 0) return null;
        return Math.Clamp((int)Math.Round(downloadedBytes / (double)totalBytes * 100, MidpointRounding.AwayFromZero), 0, 100);
    }

    private static string SafeInstallerAssetName(string assetName)
    {
        string fileName = Path.GetFileName(assetName);
        return string.IsNullOrWhiteSpace(fileName) ? InstallerAssetName("unknown") : fileName;
    }

    private static void TryDelete(string path)
    {
        try
        {
            File.Delete(path);
        }
        catch
        {
            // Best effort cleanup.
        }
    }

    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.SnakeCaseLower,
        PropertyNameCaseInsensitive = true
    };

    private sealed record GitHubRelease(
        string TagName,
        string? Name,
        string? Body,
        bool Draft,
        bool Prerelease,
        IReadOnlyList<GitHubReleaseAsset> Assets);

    private sealed record GitHubReleaseAsset(string Name, string BrowserDownloadUrl);
}

internal readonly record struct SemVersion(int Major, int Minor, int Patch, string? Prerelease) : IComparable<SemVersion>
{
    private static readonly Regex VersionPattern = new(
        @"^v?(?<major>0|[1-9]\d*)\.(?<minor>0|[1-9]\d*)\.(?<patch>0|[1-9]\d*)(?:-(?<pre>[0-9A-Za-z.-]+))?$",
        RegexOptions.Compiled | RegexOptions.CultureInvariant);

    public bool IsPrerelease => !string.IsNullOrWhiteSpace(Prerelease);

    public static SemVersion Parse(string value)
    {
        Match match = VersionPattern.Match(value);
        if (!match.Success) throw new FormatException("Invalid semantic version.");
        return new SemVersion(
            int.Parse(match.Groups["major"].Value, CultureInfo.InvariantCulture),
            int.Parse(match.Groups["minor"].Value, CultureInfo.InvariantCulture),
            int.Parse(match.Groups["patch"].Value, CultureInfo.InvariantCulture),
            match.Groups["pre"].Success ? match.Groups["pre"].Value : null);
    }

    public static bool IsNewer(string candidate, string current)
    {
        try
        {
            return Parse(candidate).CompareTo(Parse(current)) > 0;
        }
        catch
        {
            return false;
        }
    }

    public int CompareTo(SemVersion other)
    {
        int major = Major.CompareTo(other.Major);
        if (major != 0) return major;
        int minor = Minor.CompareTo(other.Minor);
        if (minor != 0) return minor;
        int patch = Patch.CompareTo(other.Patch);
        if (patch != 0) return patch;
        if (Prerelease == other.Prerelease) return 0;
        if (Prerelease is null) return 1;
        if (other.Prerelease is null) return -1;
        return ComparePrerelease(Prerelease, other.Prerelease);
    }

    private static int ComparePrerelease(string left, string right)
    {
        string[] leftParts = left.Split('.');
        string[] rightParts = right.Split('.');
        int length = Math.Max(leftParts.Length, rightParts.Length);
        for (int index = 0; index < length; index++)
        {
            if (index >= leftParts.Length) return -1;
            if (index >= rightParts.Length) return 1;

            bool leftNumeric = int.TryParse(leftParts[index], NumberStyles.None, CultureInfo.InvariantCulture, out int leftNumber);
            bool rightNumeric = int.TryParse(rightParts[index], NumberStyles.None, CultureInfo.InvariantCulture, out int rightNumber);
            if (leftNumeric && rightNumeric)
            {
                int numeric = leftNumber.CompareTo(rightNumber);
                if (numeric != 0) return numeric;
                continue;
            }

            if (leftNumeric != rightNumeric) return leftNumeric ? -1 : 1;
            int text = string.CompareOrdinal(leftParts[index], rightParts[index]);
            if (text != 0) return text;
        }

        return 0;
    }
}
