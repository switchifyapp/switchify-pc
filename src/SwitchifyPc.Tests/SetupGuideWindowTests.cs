using System.Threading;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Shell;
using SwitchifyPc.App;
using SwitchifyPc.App.Themes;
using SwitchifyPc.Core.Pairing;
using SwitchifyPc.Core.Ui;
using WpfButton = System.Windows.Controls.Button;
using WpfColor = System.Windows.Media.Color;

namespace SwitchifyPc.Tests;

[Collection(WpfTestCollection.Name)]
public sealed class SetupGuideWindowTests
{
    [Fact]
    public void LoadsBluetoothStepWithCustomChrome()
    {
        RunOnSta(() =>
        {
            WpfTestApplication.ApplyTheme(AppTheme.Light);
            SetupGuideWindow window = new(new SetupGuideViewModel());
            try
            {
                window.Show();
                window.UpdateLayout();

                Assert.Equal(WindowStyle.None, window.WindowStyle);
                Assert.Equal(ResizeMode.NoResize, window.ResizeMode);
                Assert.NotNull(WindowChrome.GetWindowChrome(window));
                Assert.Contains("Check Bluetooth", TextBlocks(window));
                Assert.Contains("Step 1 of 5", TextBlocks(window));
                Assert.NotNull(ButtonByAutomationName(window, "Minimize"));
                Assert.NotNull(ButtonByAutomationName(window, "Close"));
                SolidColorBrush chromeForeground = Assert.IsType<SolidColorBrush>(window.FindResource("ChromeForeground"));
                Assert.Equal(chromeForeground.Color, Assert.IsType<SolidColorBrush>(TextBlockByText(window, "Setup guide").Foreground).Color);
                Assert.Equal(chromeForeground.Color, Assert.IsType<SolidColorBrush>(TextBlockByText(window, "Connect Switchify for Android").Foreground).Color);
                Assert.Equal(chromeForeground.Color, Assert.IsType<SolidColorBrush>(TextBlockByText(window, "Step 1 of 5").Foreground).Color);
            }
            finally
            {
                window.Close();
            }
        });
    }

    [Fact]
    public void LoadsAndroidQrAndSecurePairingApproval()
    {
        RunOnSta(() =>
        {
            WpfTestApplication.ApplyTheme(AppTheme.Light);
            SetupGuideViewModel viewModel = new();
            viewModel.MoveNext();
            SetupGuideWindow window = new(viewModel);
            try
            {
                window.Show();
                window.UpdateLayout();

                Assert.Contains("Get Switchify for Android", TextBlocks(window));
                string assemblyName = Uri.EscapeDataString(typeof(SetupGuideWindow).Assembly.GetName().Name ?? "Switchify PC");
                Uri qrUri = new($"pack://application:,,,/{assemblyName};component/Assets/android-download-qr.png", UriKind.Absolute);
                Assert.NotNull(System.Windows.Application.GetResourceStream(qrUri));

                viewModel.MoveNext();
                viewModel.SetPairingApprovals([
                    new PendingPairingApprovalView("request-1", "Pixel 9", "123456", 1, 2, null)
                ]);
                window.UpdateLayout();

                Assert.Contains("Pair securely", TextBlocks(window));
                Assert.Contains("Pixel 9", TextBlocks(window));
                Assert.Contains("Verification code 123456", TextBlocks(window));
                Assert.Contains("Accept", ButtonContent(window));
                Assert.Contains("Reject", ButtonContent(window));
            }
            finally
            {
                window.Close();
            }
        });
    }

    [Fact]
    public void LoadsWithDarkTheme()
    {
        RunOnSta(() =>
        {
            WpfTestApplication.ApplyTheme(AppTheme.Dark);
            SetupGuideWindow window = new(new SetupGuideViewModel());
            try
            {
                window.Show();
                window.UpdateLayout();

                Assert.Contains("Connect Switchify for Android", TextBlocks(window));
                SolidColorBrush surface = Assert.IsType<SolidColorBrush>(window.FindResource("Surface"));
                Assert.Equal(WpfColor.FromRgb(0x1F, 0x1F, 0x23), surface.Color);
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
            if (node is TextBlock textBlock) text.Add(textBlock.Text);
        });
        return text;
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

    private static TextBlock TextBlockByText(DependencyObject root, string text)
    {
        TextBlock? result = null;
        Collect(root, node =>
        {
            if (result is null && node is TextBlock textBlock && textBlock.Text == text)
            {
                result = textBlock;
            }
        });
        return Assert.IsType<TextBlock>(result);
    }

    private static WpfButton? ButtonByAutomationName(DependencyObject root, string name)
    {
        WpfButton? result = null;
        Collect(root, node =>
        {
            if (result is null && node is WpfButton button && AutomationProperties.GetName(button) == name)
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
}
