using System.Threading;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Media;
using SwitchifyPc.App.Chrome;
using WpfButton = System.Windows.Controls.Button;

namespace SwitchifyPc.Tests;

[Collection(WpfTestCollection.Name)]
public sealed class SwitchifyTitleBarTests
{
    [Fact]
    public void ExposesTitleText()
    {
        RunOnSta(() =>
        {
            SwitchifyTitleBar titleBar = new()
            {
                TitleText = "Switchify PC test"
            };
            Window window = new()
            {
                Content = titleBar,
                Width = 320,
                Height = 120
            };
            try
            {
                window.Show();
                window.UpdateLayout();

                Assert.Contains("Switchify PC test", TextBlocks(window));
            }
            finally
            {
                window.Close();
            }
        });
    }

    [Fact]
    public void MinimizeButtonSetsParentWindowState()
    {
        RunOnSta(() =>
        {
            SwitchifyTitleBar titleBar = new();
            Window window = new()
            {
                Content = titleBar,
                Width = 320,
                Height = 120
            };
            try
            {
                window.Show();
                window.UpdateLayout();

                WpfButton minimize = Assert.IsType<WpfButton>(ButtonByAutomationName(window, "Minimize"));
                minimize.RaiseEvent(new RoutedEventArgs(WpfButton.ClickEvent));

                Assert.Equal(WindowState.Minimized, window.WindowState);
            }
            finally
            {
                window.Close();
            }
        });
    }

    [Fact]
    public void CloseButtonInvokesParentWindowClose()
    {
        RunOnSta(() =>
        {
            bool closingCalled = false;
            SwitchifyTitleBar titleBar = new();
            Window window = new()
            {
                Content = titleBar,
                Width = 320,
                Height = 120
            };
            window.Closing += (_, _) => closingCalled = true;
            try
            {
                window.Show();
                window.UpdateLayout();

                WpfButton close = Assert.IsType<WpfButton>(ButtonByAutomationName(window, "Close"));
                close.RaiseEvent(new RoutedEventArgs(WpfButton.ClickEvent));

                Assert.True(closingCalled);
            }
            finally
            {
                if (window.IsVisible)
                {
                    window.Close();
                }
            }
        });
    }

    [Fact]
    public void CanHideMinimizeButton()
    {
        RunOnSta(() =>
        {
            SwitchifyTitleBar titleBar = new()
            {
                ShowMinimizeButton = false
            };
            Window window = new()
            {
                Content = titleBar,
                Width = 320,
                Height = 120
            };
            try
            {
                window.Show();
                window.UpdateLayout();

                WpfButton minimize = Assert.IsType<WpfButton>(ButtonByAutomationName(window, "Minimize"));
                Assert.Equal(Visibility.Collapsed, minimize.Visibility);
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
