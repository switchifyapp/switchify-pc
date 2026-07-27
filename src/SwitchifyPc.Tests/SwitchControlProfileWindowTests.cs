using System.Threading;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Shell;
using SwitchifyPc.App;
using SwitchifyPc.App.Themes;
using SwitchifyPc.Core.SwitchControl;
using WpfButton = System.Windows.Controls.Button;
using WpfColor = System.Windows.Media.Color;

namespace SwitchifyPc.Tests;

[Collection(WpfTestCollection.Name)]
public sealed class SwitchControlProfileWindowTests
{
    [Fact]
    public void ProfileWindowUsesSwitchifyChromeAndFixedLayout()
    {
        RunOnSta(() =>
        {
            WpfTestApplication.ApplyTheme(AppTheme.Light);
            SwitchControlProfileWindow window = CreateWindow();
            try
            {
                window.Show();
                window.UpdateLayout();

                Assert.Equal(WindowStyle.None, window.WindowStyle);
                Assert.Equal(ResizeMode.NoResize, window.ResizeMode);
                Assert.Equal(980, window.Width);
                Assert.Equal(720, window.Height);
                Assert.NotNull(WindowChrome.GetWindowChrome(window));
                Assert.Contains("PC Switch Control profiles", TextBlocks(window));
                Assert.NotNull(ButtonByAutomationName(window, "Minimize"));
                Assert.NotNull(ButtonByAutomationName(window, "Close"));

                WpfButton save = Assert.IsType<WpfButton>(window.FindName("SaveButton"));
                Assert.Same(window.FindResource("PrimaryButton"), save.Style);
            }
            finally
            {
                window.Close();
            }
        });
    }

    [Fact]
    public void ProfileWindowLoadsWithDarkTheme()
    {
        RunOnSta(() =>
        {
            WpfTestApplication.ApplyTheme(AppTheme.Dark);
            SwitchControlProfileWindow window = CreateWindow();
            try
            {
                window.Show();
                window.UpdateLayout();

                SolidColorBrush appBackground = Assert.IsType<SolidColorBrush>(window.FindResource("AppBackground"));
                Assert.Equal(WpfColor.FromRgb(0x14, 0x13, 0x18), appBackground.Color);
                Assert.Contains("Profile details", TextBlocks(window));
                Assert.Contains("Switch bindings", TextBlocks(window));
            }
            finally
            {
                window.Close();
            }
        });
    }

    private static SwitchControlProfileWindow CreateWindow() =>
        new(new StaticProfileStore(), () => null);

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
        Collect(root, node =>
        {
            if (node is TextBlock textBlock) text.Add(textBlock.Text);
        });
        return text;
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

    private static void Collect(DependencyObject node, Action<DependencyObject> visit)
    {
        visit(node);
        for (int index = 0; index < VisualTreeHelper.GetChildrenCount(node); index++)
        {
            Collect(VisualTreeHelper.GetChild(node, index), visit);
        }
    }

    private sealed class StaticProfileStore : ISwitchControlProfileStore
    {
        public IReadOnlyList<SwitchControlProfile> Load() => SwitchControlProfiles.BuiltIns;

        public IReadOnlyList<SwitchControlProfile> Save(IReadOnlyList<SwitchControlProfile> customProfiles) =>
            [.. SwitchControlProfiles.BuiltIns, .. customProfiles];
    }
}
