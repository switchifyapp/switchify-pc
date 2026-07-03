using System.Threading;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Shell;
using SwitchifyPc.App;
using SwitchifyPc.App.Themes;
using SwitchifyPc.Core.Ui;
using WpfButton = System.Windows.Controls.Button;
using WpfColor = System.Windows.Media.Color;

namespace SwitchifyPc.Tests;

[Collection(WpfTestCollection.Name)]
public sealed class MainWindowTests
{
    [Fact]
    public void MainWindowLoadsWithAndroidDownloadPrompt()
    {
        RunOnSta(() =>
        {
            WpfTestApplication.ApplyTheme(AppTheme.Light);
            MainWindowViewModel viewModel = new();
            viewModel.SetAndroidDownloadPromptState(dismissed: false, hasPairedDevices: false);
            MainWindow window = new(viewModel);
            try
            {
                window.Show();
                window.UpdateLayout();

                string assemblyName = Uri.EscapeDataString(typeof(MainWindow).Assembly.GetName().Name ?? "Switchify PC");
                Uri qrUri = new($"pack://application:,,,/{assemblyName};component/Assets/android-download-qr.png", UriKind.Absolute);
                Assert.NotNull(System.Windows.Application.GetResourceStream(qrUri));
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

    private static void Collect(DependencyObject node, Action<DependencyObject> visit)
    {
        visit(node);
        for (int index = 0; index < VisualTreeHelper.GetChildrenCount(node); index++)
        {
            Collect(VisualTreeHelper.GetChild(node, index), visit);
        }
    }
}
