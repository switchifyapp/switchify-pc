using System.Threading;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Shell;
using SwitchifyPc.App;
using SwitchifyPc.App.Chrome;
using SwitchifyPc.App.Themes;
using SwitchifyPc.Core.Bluetooth;
using SwitchifyPc.Core.Ui;
using WpfButton = System.Windows.Controls.Button;
using WpfColor = System.Windows.Media.Color;

namespace SwitchifyPc.Tests;

[Collection(WpfTestCollection.Name)]
public sealed class MainWindowTests
{
    [Fact]
    public void MainWindowShowsSetupGuideEntryWithoutLegacyDownloadCard()
    {
        RunOnSta(() =>
        {
            WpfTestApplication.ApplyTheme(AppTheme.Light);
            MainWindow window = new(new MainWindowViewModel());
            try
            {
                window.Show();
                window.UpdateLayout();

                Assert.Contains("Setup guide", ButtonContent(window));
                Assert.DoesNotContain("Get Switchify for Android", TextBlocks(window));
            }
            finally
            {
                window.Close();
            }
        });
    }

    [Fact]
    public void MainWindowUsesCustomChrome()
    {
        RunOnSta(() =>
        {
            WpfTestApplication.ApplyTheme(AppTheme.Light);
            MainWindow window = new(new MainWindowViewModel());
            try
            {
                window.Show();
                window.UpdateLayout();

                Assert.Equal(WindowStyle.None, window.WindowStyle);
                Assert.NotNull(WindowChrome.GetWindowChrome(window));
                Assert.Contains("Switchify PC", TextBlocks(window));
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
    public void MainWindowLoadsWithDarkTheme()
    {
        RunOnSta(() =>
        {
            WpfTestApplication.ApplyTheme(AppTheme.Dark);
            MainWindow window = new(new MainWindowViewModel());
            try
            {
                window.Show();
                window.UpdateLayout();

                Assert.Equal(WindowStyle.None, window.WindowStyle);
                Assert.Contains("Switchify PC", TextBlocks(window));
                SolidColorBrush surface = Assert.IsType<SolidColorBrush>(window.FindResource("Surface"));
                Assert.Equal(WpfColor.FromRgb(0x1F, 0x1F, 0x23), surface.Color);
            }
            finally
            {
                window.Close();
            }
        });
    }

    [Fact]
    public void MainWindowShowsConnectionStatusInTitleBar()
    {
        RunOnSta(() =>
        {
            WpfTestApplication.ApplyTheme(AppTheme.Light);
            MainWindowViewModel viewModel = new();
            viewModel.SetBluetoothState(
                DesktopUiState.Connected,
                BluetoothStatusModel.DefaultStatus with
                {
                    Status = "connected",
                    ConnectedClientCount = 1,
                    System = BluetoothStatusModel.DefaultSystemStatus with
                    {
                        AdapterPresent = true,
                        RadioState = "on",
                        IsLowEnergySupported = true,
                        IsPeripheralRoleSupported = true
                    }
                });

            MainWindow window = new(viewModel);
            try
            {
                window.Show();
                window.UpdateLayout();

                FrameworkElement badge = Assert.IsType<Border>(ElementByAutomationId(window, "TitleBarStatusBadge"));
                Assert.Equal(Visibility.Visible, badge.Visibility);
                Assert.Equal("Connected", AutomationProperties.GetName(badge));

                SwitchifyTitleBar titleBar = Assert.IsType<SwitchifyTitleBar>(Ancestor<SwitchifyTitleBar>(badge));
                Assert.Contains("Connected", TextBlocks(titleBar));
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
        Collect(root, node =>
        {
            if (node is TextBlock textBlock)
            {
                text.Add(textBlock.Text);
            }
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

    private static IReadOnlyList<string> ButtonContent(DependencyObject root)
    {
        List<string> content = [];
        Collect(root, node =>
        {
            if (node is WpfButton { Content: string text }) content.Add(text);
        });
        return content;
    }

    private static FrameworkElement? ElementByAutomationId(DependencyObject root, string automationId)
    {
        FrameworkElement? result = null;
        Collect(root, node =>
        {
            if (result is null &&
                node is FrameworkElement element &&
                AutomationProperties.GetAutomationId(element) == automationId)
            {
                result = element;
            }
        });
        return result;
    }

    private static T? Ancestor<T>(DependencyObject node)
        where T : DependencyObject
    {
        DependencyObject? current = node;
        while (current is not null)
        {
            if (current is T match)
            {
                return match;
            }

            current = VisualTreeHelper.GetParent(current);
        }

        return null;
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
