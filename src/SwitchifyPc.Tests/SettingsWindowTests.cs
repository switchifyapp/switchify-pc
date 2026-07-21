using System.Threading;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Shell;
using SwitchifyPc.App;
using SwitchifyPc.App.Themes;
using SwitchifyPc.Core.Ui;
using SwitchifyPc.Core.Updates;
using WpfButton = System.Windows.Controls.Button;
using WpfColor = System.Windows.Media.Color;

namespace SwitchifyPc.Tests;

[Collection(WpfTestCollection.Name)]
public sealed class SettingsWindowTests
{
    [Fact]
    public void SettingsWindowUsesOneClickUpdateCopy()
    {
        RunOnSta(() =>
        {
            WpfTestApplication.ApplyTheme(AppTheme.Light);
            SettingsWindow window = new(new SettingsViewModel());
            try
            {
                window.Show();
                window.UpdateLayout();

                Assert.Contains("Check for and install Switchify PC updates.", TextBlocks(window));
                Assert.Contains("Check for updates", ButtonContent(window));
                Assert.DoesNotContain("Download update", ButtonContent(window));
            }
            finally
            {
                window.Close();
            }
        });
    }

    [Fact]
    public void SettingsWindowLoadsWithUpdateDownloadProgress()
    {
        RunOnSta(() =>
        {
            WpfTestApplication.ApplyTheme(AppTheme.Light);
            SettingsViewModel viewModel = new();
            viewModel.SetUpdateState(UpdateState.CreateInitial("0.2.4") with
            {
                Download = new UpdateDownloadProgress(
                    UpdateDownloadStatus.Downloading,
                    20_971_520,
                    52_428_800,
                    40)
            });

            SettingsWindow window = new(viewModel);
            try
            {
                window.Show();
                window.UpdateLayout();
            }
            finally
            {
                window.Close();
            }
        });
    }

    [Fact]
    public void SettingsWindowShowsSeparateMouseRepeatIntervals()
    {
        RunOnSta(() =>
        {
            WpfTestApplication.ApplyTheme(AppTheme.Light);
            SettingsViewModel viewModel = new();
            SettingsWindow window = new(viewModel);
            try
            {
                window.Show();
                window.UpdateLayout();

                IReadOnlyList<string> text = TextBlocks(window);
                Assert.Contains("Movement repeat interval", text);
                Assert.Contains("Time between repeated pointer movements.", text);
                Assert.Contains("Scroll repeat interval", text);
                Assert.Contains("Time between repeated scroll actions.", text);
                Assert.Contains("Movement acceleration", text);
                Assert.Contains("Gradually increase repeated movement to the selected pointer speed.", text);
                IReadOnlyList<string> radioButtons = RadioButtonContent(window);
                Assert.Contains("Off", radioButtons);
                Assert.Contains("Short", radioButtons);
                Assert.Contains("Medium", radioButtons);
                Assert.Contains("Long", radioButtons);
                Assert.DoesNotContain("Repeat Interval", text);
            }
            finally
            {
                window.Close();
            }
        });
    }

    [Fact]
    public void SettingsWindowShowsFivePercentPointerScale()
    {
        RunOnSta(() =>
        {
            WpfTestApplication.ApplyTheme(AppTheme.Light);
            SettingsWindow window = new(new SettingsViewModel());
            try
            {
                window.Show();
                window.UpdateLayout();

                Assert.Contains("5%", RadioButtonContent(window));
            }
            finally
            {
                window.Close();
            }
        });
    }

    [Fact]
    public void SettingsWindowUsesPointerSpeedCopy()
    {
        RunOnSta(() =>
        {
            WpfTestApplication.ApplyTheme(AppTheme.Light);
            SettingsWindow window = new(new SettingsViewModel());
            try
            {
                window.Show();
                window.UpdateLayout();

                IReadOnlyList<string> text = TextBlocks(window);
                Assert.Contains("Tune pointer speed and the visual cursor marker used while controlling this PC.", text);
                Assert.Contains("Pointer speed", text);
                Assert.Contains("Choose how quickly Android pointer movement moves on this display.", text);
                Assert.DoesNotContain("Tune movement distance and the visual cursor marker used while controlling this PC.", text);
                Assert.DoesNotContain("Movement distance", text);
                Assert.DoesNotContain("Choose how far each Android pointer step moves on this display.", text);
            }
            finally
            {
                window.Close();
            }
        });
    }

    [Fact]
    public void SettingsWindowUsesCustomChrome()
    {
        RunOnSta(() =>
        {
            WpfTestApplication.ApplyTheme(AppTheme.Light);
            SettingsWindow window = new(new SettingsViewModel());
            try
            {
                window.Show();
                window.UpdateLayout();

                Assert.Equal(WindowStyle.None, window.WindowStyle);
                Assert.Equal(ResizeMode.NoResize, window.ResizeMode);
                Assert.NotNull(WindowChrome.GetWindowChrome(window));
                Assert.Contains("Switchify PC settings", TextBlocks(window));
                Assert.NotNull(ButtonByAutomationName(window, "Minimize"));
                Assert.NotNull(ButtonByAutomationName(window, "Close"));
            }
            finally
            {
                window.Close();
            }
        });
    }

    [Fact]
    public void SettingsWindowLoadsWithDarkTheme()
    {
        RunOnSta(() =>
        {
            WpfTestApplication.ApplyTheme(AppTheme.Dark);
            SettingsWindow window = new(new SettingsViewModel());
            try
            {
                window.Show();
                window.UpdateLayout();

                Assert.Contains("Switchify PC settings", TextBlocks(window));
                Assert.Contains("Settings", TextBlocks(window));
                SolidColorBrush appBackground = Assert.IsType<SolidColorBrush>(window.FindResource("AppBackground"));
                Assert.Equal(WpfColor.FromRgb(0x14, 0x13, 0x18), appBackground.Color);
            }
            finally
            {
                window.Close();
            }
        });
    }

    private static void RunOnSta(Action action)
    {
        Exception? exception = null;
        Thread thread = new(() =>
        {
            try
            {
                action();
            }
            catch (Exception error)
            {
                exception = error;
            }
        });

        thread.SetApartmentState(ApartmentState.STA);
        thread.Start();
        thread.Join();

        if (exception is not null) throw exception;
    }

    private static IReadOnlyList<string> TextBlocks(DependencyObject root)
    {
        List<string> text = [];
        CollectTextBlocks(root, text);
        return text;
    }

    private static void CollectTextBlocks(DependencyObject node, List<string> text)
    {
        if (node is TextBlock textBlock)
        {
            text.Add(textBlock.Text);
        }

        for (int index = 0; index < VisualTreeHelper.GetChildrenCount(node); index++)
        {
            CollectTextBlocks(VisualTreeHelper.GetChild(node, index), text);
        }
    }

    private static WpfButton? ButtonByAutomationName(DependencyObject root, string name)
    {
        WpfButton? result = null;
        Collect(root, node =>
        {
            if (result is null &&
                node is WpfButton button &&
                AutomationProperties.GetName(button) == name)
            {
                result = button;
            }
        });
        return result;
    }

    private static IReadOnlyList<string> RadioButtonContent(DependencyObject root)
    {
        List<string> content = [];
        Collect(root, node =>
        {
            if (node is System.Windows.Controls.RadioButton { Content: string text })
            {
                content.Add(text);
            }
        });
        return content;
    }

    private static IReadOnlyList<string> ButtonContent(DependencyObject root)
    {
        List<string> content = [];
        Collect(root, node =>
        {
            if (node is WpfButton { Content: string text }) content.Add(text);
        });
        return content;
    }

    private static void Collect(DependencyObject node, Action<DependencyObject> visit)
    {
        visit(node);
        for (int index = 0; index < VisualTreeHelper.GetChildrenCount(node); index++)
        {
            Collect(VisualTreeHelper.GetChild(node, index), visit);
        }
    }
}
