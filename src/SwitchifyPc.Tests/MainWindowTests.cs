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
using SwitchifyPc.Core.Pairing;
using SwitchifyPc.Core.Ui;
using SwitchifyPc.Core.Updates;
using WpfButton = System.Windows.Controls.Button;
using WpfColor = System.Windows.Media.Color;

namespace SwitchifyPc.Tests;

[Collection(WpfTestCollection.Name)]
public sealed class MainWindowTests
{
    [Fact]
    public void UpdateBannerRunsInstallActionDirectly()
    {
        RunOnSta(() =>
        {
            WpfTestApplication.ApplyTheme(AppTheme.Light);
            MainWindowViewModel viewModel = new();
            UpdateState initial = UpdateState.CreateInitial("0.2.0");
            viewModel.SetUpdateState(initial with
            {
                Info = initial.Info with { Status = UpdateCheckStatus.UpdateAvailable, LatestVersion = "0.2.1" }
            });
            int calls = 0;
            MainWindow window = new(
                viewModel,
                installUpdate: () =>
                {
                    calls++;
                    return Task.FromResult(UpdateApplyResult.Success());
                });
            try
            {
                window.Show();
                window.UpdateLayout();
                WpfButton button = Assert.IsType<WpfButton>(ButtonByContent(window, "Install update"));

                button.RaiseEvent(new RoutedEventArgs(WpfButton.ClickEvent));

                Assert.Equal(1, calls);
            }
            finally
            {
                window.Close();
            }
        });
    }

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

    [Fact]
    public void PairingApprovalsReplaceConnectedDeviceUiAndPreserveAcceptRequest()
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
                        RadioState = "on"
                    }
                });
            viewModel.SetPairingApprovals([
                new PendingPairingApprovalView("approval-1", "Pixel 9", "123456", 1, 2, null),
                new PendingPairingApprovalView("approval-2", "Galaxy Tab", "654321", 3, 4, null)
            ]);
            string? acceptedRequestId = null;
            MainWindow window = new(
                viewModel,
                acceptPairingApproval: requestId =>
                {
                    acceptedRequestId = requestId;
                    return Task.CompletedTask;
                });
            try
            {
                window.Show();
                window.UpdateLayout();

                FrameworkElement pairingPanel = Assert.Single(ElementsByAutomationId(window, "PairingApprovalsPanel"));
                FrameworkElement connectedPanel = Assert.IsType<Border>(ElementByAutomationId(window, "ConnectedDevicePanel"));
                WpfButton disconnectButton = Assert.IsType<WpfButton>(ElementByAutomationId(window, "DisconnectDeviceButton"));

                Assert.Equal(Visibility.Visible, pairingPanel.Visibility);
                Assert.Equal(Visibility.Collapsed, connectedPanel.Visibility);
                Assert.Equal(Visibility.Collapsed, disconnectButton.Visibility);
                Assert.Contains("Pixel 9", TextBlocks(pairingPanel));
                Assert.Contains("Verification code 123456", TextBlocks(pairingPanel));
                Assert.Contains("Galaxy Tab", TextBlocks(pairingPanel));
                Assert.Contains("Verification code 654321", TextBlocks(pairingPanel));

                WpfButton acceptSecond = Assert.IsType<WpfButton>(ButtonByContentAndTag(pairingPanel, "Accept", "approval-2"));
                WpfButton rejectSecond = Assert.IsType<WpfButton>(ButtonByContentAndTag(pairingPanel, "Reject", "approval-2"));
                Assert.Equal("Accept pairing request from Galaxy Tab", AutomationProperties.GetName(acceptSecond));
                Assert.Equal("Reject pairing request from Galaxy Tab", AutomationProperties.GetName(rejectSecond));
                acceptSecond.RaiseEvent(new RoutedEventArgs(WpfButton.ClickEvent));
                Assert.Equal("approval-2", acceptedRequestId);

                viewModel.SetPairingApprovals([]);
                window.UpdateLayout();

                Assert.Equal(Visibility.Collapsed, pairingPanel.Visibility);
                Assert.Equal(Visibility.Visible, connectedPanel.Visibility);
                Assert.Equal(Visibility.Visible, disconnectButton.Visibility);
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

    private static WpfButton? ButtonByContent(DependencyObject root, string content)
    {
        WpfButton? result = null;
        Collect(root, node =>
        {
            if (result is null && node is WpfButton { Content: string text } button && text == content)
            {
                result = button;
            }
        });
        return result;
    }

    private static WpfButton? ButtonByContentAndTag(DependencyObject root, string content, string tag)
    {
        WpfButton? result = null;
        Collect(root, node =>
        {
            if (result is null &&
                node is WpfButton { Content: string text, Tag: string requestId } button &&
                text == content &&
                requestId == tag)
            {
                result = button;
            }
        });
        return result;
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

    private static IReadOnlyList<FrameworkElement> ElementsByAutomationId(DependencyObject root, string automationId)
    {
        List<FrameworkElement> results = [];
        Collect(root, node =>
        {
            if (node is FrameworkElement element &&
                AutomationProperties.GetAutomationId(element) == automationId)
            {
                results.Add(element);
            }
        });
        return results;
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
