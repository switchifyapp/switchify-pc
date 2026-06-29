using System.Globalization;

namespace SwitchifyPc.Core.Storage;

public sealed record CorruptJsonBackupResult(string? BackupPath);

public static class JsonFileStore
{
    public static async Task WriteJsonFileAtomicAsync(string filePath, string content, CancellationToken cancellationToken = default)
    {
        Directory.CreateDirectory(Path.GetDirectoryName(filePath) ?? ".");
        string tempPath = TempPathFor(filePath);
        var renamed = false;

        try
        {
            await using (FileStream stream = new(
                tempPath,
                FileMode.Create,
                FileAccess.Write,
                FileShare.None,
                bufferSize: 4096,
                FileOptions.WriteThrough | FileOptions.Asynchronous))
            {
                await using StreamWriter writer = new(stream, System.Text.Encoding.UTF8);
                await writer.WriteAsync(content.AsMemory(), cancellationToken);
                await writer.FlushAsync(cancellationToken);
                stream.Flush(flushToDisk: true);
            }

            File.Move(tempPath, filePath, overwrite: true);
            renamed = true;
        }
        finally
        {
            if (!renamed)
            {
                TryDelete(tempPath);
            }
        }
    }

    public static void WriteJsonFileAtomicSync(string filePath, string content)
    {
        Directory.CreateDirectory(Path.GetDirectoryName(filePath) ?? ".");
        string tempPath = TempPathFor(filePath);
        var renamed = false;

        try
        {
            using (FileStream stream = new(tempPath, FileMode.Create, FileAccess.Write, FileShare.None, 4096, FileOptions.WriteThrough))
            using (StreamWriter writer = new(stream, System.Text.Encoding.UTF8))
            {
                writer.Write(content);
                writer.Flush();
                stream.Flush(flushToDisk: true);
            }

            File.Move(tempPath, filePath, overwrite: true);
            renamed = true;
        }
        finally
        {
            if (!renamed)
            {
                TryDelete(tempPath);
            }
        }
    }

    public static Task<CorruptJsonBackupResult> BackupCorruptJsonFileAsync(string filePath)
    {
        if (!File.Exists(filePath))
        {
            return Task.FromResult(new CorruptJsonBackupResult(null));
        }

        string backupPath = CorruptBackupPathFor(filePath);

        try
        {
            File.Move(filePath, backupPath);
            return Task.FromResult(new CorruptJsonBackupResult(backupPath));
        }
        catch (FileNotFoundException)
        {
            return Task.FromResult(new CorruptJsonBackupResult(null));
        }
        catch (DirectoryNotFoundException)
        {
            return Task.FromResult(new CorruptJsonBackupResult(null));
        }
    }

    internal static string TempPathFor(string filePath)
    {
        return $"{filePath}.{Environment.ProcessId}.{DateTimeOffset.UtcNow.ToUnixTimeMilliseconds()}.{Guid.NewGuid():N}.tmp";
    }

    internal static string CorruptBackupPathFor(string filePath, DateTimeOffset? now = null)
    {
        string directory = Path.GetDirectoryName(filePath) ?? ".";
        string extension = Path.GetExtension(filePath);
        string name = Path.GetFileNameWithoutExtension(filePath);
        string timestamp = SanitizeTimestamp((now ?? DateTimeOffset.UtcNow).UtcDateTime.ToString("yyyy-MM-ddTHH:mm:ss.fffZ", CultureInfo.InvariantCulture));
        return Path.Combine(directory, $"{name}.corrupt-{timestamp}{extension}");
    }

    internal static string SanitizeTimestamp(string value)
    {
        return value.Replace("-", "", StringComparison.Ordinal)
            .Replace(":", "", StringComparison.Ordinal)
            .Replace(".", "", StringComparison.Ordinal);
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
}
