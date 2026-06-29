using SwitchifyPc.Core.Storage;

namespace SwitchifyPc.Tests;

public sealed class JsonFileStoreTests : IDisposable
{
    private readonly string tempDir = Path.Combine(Path.GetTempPath(), $"switchify-json-store-{Guid.NewGuid():N}");

    public void Dispose()
    {
        if (Directory.Exists(tempDir))
        {
            Directory.Delete(tempDir, recursive: true);
        }
    }

    [Fact]
    public async Task WriteJsonFileAtomicWritesContentToTargetPath()
    {
        string filePath = Path.Combine(tempDir, "settings.json");

        await JsonFileStore.WriteJsonFileAtomicAsync(filePath, "{\"ok\":true}\n");

        Assert.Equal("{\"ok\":true}\n", await File.ReadAllTextAsync(filePath));
    }

    [Fact]
    public async Task WriteJsonFileAtomicCreatesParentDirectories()
    {
        string filePath = Path.Combine(tempDir, "nested", "settings.json");

        await JsonFileStore.WriteJsonFileAtomicAsync(filePath, "{}\n");

        Assert.True(File.Exists(filePath));
    }

    [Fact]
    public async Task WriteJsonFileAtomicLeavesNoTempFileAfterSuccess()
    {
        string filePath = Path.Combine(tempDir, "settings.json");

        await JsonFileStore.WriteJsonFileAtomicAsync(filePath, "{}\n");

        Assert.Empty(Directory.EnumerateFiles(tempDir, "*.tmp"));
    }

    [Fact]
    public void WriteJsonFileAtomicSyncWritesContentToTargetPath()
    {
        string filePath = Path.Combine(tempDir, "settings.json");

        JsonFileStore.WriteJsonFileAtomicSync(filePath, "{\"ok\":true}\n");

        Assert.Equal("{\"ok\":true}\n", File.ReadAllText(filePath));
    }

    [Fact]
    public async Task BackupCorruptJsonFileRenamesExistingFile()
    {
        Directory.CreateDirectory(tempDir);
        string filePath = Path.Combine(tempDir, "pairing-state.json");
        await File.WriteAllTextAsync(filePath, "{");

        CorruptJsonBackupResult result = await JsonFileStore.BackupCorruptJsonFileAsync(filePath);

        Assert.NotNull(result.BackupPath);
        Assert.False(File.Exists(filePath));
        Assert.True(File.Exists(result.BackupPath));
        Assert.Contains("pairing-state.corrupt-", Path.GetFileName(result.BackupPath), StringComparison.Ordinal);
        Assert.DoesNotContain(':', Path.GetFileName(result.BackupPath));
    }

    [Fact]
    public async Task BackupCorruptJsonFileReturnsNullForMissingFile()
    {
        CorruptJsonBackupResult result = await JsonFileStore.BackupCorruptJsonFileAsync(Path.Combine(tempDir, "missing.json"));

        Assert.Null(result.BackupPath);
    }
}
