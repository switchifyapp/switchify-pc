namespace SwitchifyPc.Tests;

public sealed class ThemeXamlGuardTests
{
    private static readonly string[] ForbiddenLightThemeLiterals =
    [
        "#F5F4F7",
        "#FFFFFF",
        "#F3F2F6",
        "#1C1B1F",
        "#49454F",
        "#E2E0E8"
    ];

    [Theory]
    [InlineData("src/SwitchifyPc.App/MainWindow.xaml")]
    [InlineData("src/SwitchifyPc.App/SettingsWindow.xaml")]
    [InlineData("src/SwitchifyPc.App/Chrome/SwitchifyTitleBar.xaml")]
    public void ThemedXamlDoesNotReintroduceHardCodedLightThemeColors(string path)
    {
        string[] lines = File.ReadAllLines(Path.Combine(RepoRoot(), path));

        for (int index = 0; index < lines.Length; index++)
        {
            string line = lines[index];
            if (IsAllowedLiteral(line)) continue;

            foreach (string forbidden in ForbiddenLightThemeLiterals)
            {
                Assert.DoesNotContain(forbidden, line, StringComparison.OrdinalIgnoreCase);
            }
        }
    }

    private static bool IsAllowedLiteral(string line)
    {
        return line.Contains("Background=\"#FFFFFF\"", StringComparison.Ordinal) &&
            line.Contains("IsCursorOverlayColorWhite", StringComparison.Ordinal) is false;
    }

    private static string RepoRoot()
    {
        DirectoryInfo? directory = new(AppContext.BaseDirectory);
        while (directory is not null)
        {
            if (Directory.Exists(Path.Combine(directory.FullName, "src", "SwitchifyPc.App")))
            {
                return directory.FullName;
            }

            directory = directory.Parent;
        }

        throw new DirectoryNotFoundException("Could not locate repository root.");
    }
}
