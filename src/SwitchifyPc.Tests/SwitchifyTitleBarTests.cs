using System.Threading;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Shapes;
using SwitchifyPc.App.Chrome;
using SwitchifyPc.App.Themes;
using WpfButton = System.Windows.Controls.Button;
using WpfColor = System.Windows.Media.Color;

namespace SwitchifyPc.Tests;

[Collection(WpfTestCollection.Name)]
public sealed class SwitchifyTitleBarTests
{
    [Fact]
    public void ExposesTitleText()
    {
        RunOnSta(() =>
        {
            WpfTestApplication.ApplyTheme(AppTheme.Light);
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
            WpfTestApplication.ApplyTheme(AppTheme.Light);
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
            WpfTestApplication.ApplyTheme(AppTheme.Light);
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
            WpfTestApplication.ApplyTheme(AppTheme.Light);
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

    [Fact]
    public void StatusBadgeIsHiddenByDefault()
    {
        RunOnSta(() =>
        {
            WpfTestApplication.ApplyTheme(AppTheme.Light);
            SwitchifyTitleBar titleBar = new();
            Window window = new()
            {
                Content = titleBar,
                Width = 420,
                Height = 120
            };
            try
            {
                window.Show();
                window.UpdateLayout();

                FrameworkElement? badge = ElementByAutomationId(window, "TitleBarStatusBadge");
                Assert.True(badge is null || badge.Visibility != Visibility.Visible);
            }
            finally
            {
                window.Close();
            }
        });
    }

    [Fact]
    public void ShowsStatusBadgeWhenEnabled()
    {
        RunOnSta(() =>
        {
            WpfTestApplication.ApplyTheme(AppTheme.Light);
            SwitchifyTitleBar titleBar = new()
            {
                ShowStatusBadge = true,
                StatusText = "Connected",
                StatusTone = "connected"
            };
            Window window = new()
            {
                Content = titleBar,
                Width = 420,
                Height = 120
            };
            try
            {
                window.Show();
                window.UpdateLayout();

                Assert.Contains("Connected", TextBlocks(window));
                FrameworkElement badge = Assert.IsType<Border>(ElementByAutomationId(window, "TitleBarStatusBadge"));
                Assert.Equal(Visibility.Visible, badge.Visibility);
                Assert.Equal("Connected", AutomationProperties.GetName(badge));
            }
            finally
            {
                window.Close();
            }
        });
    }

    [Fact]
    public void StatusBadgeUsesWarningTone()
    {
        RunOnSta(() =>
        {
            WpfTestApplication.ApplyTheme(AppTheme.Light);
            SwitchifyTitleBar titleBar = new()
            {
                ShowStatusBadge = true,
                StatusText = "Starting...",
                StatusTone = "waiting"
            };
            Window window = new()
            {
                Content = titleBar,
                Width = 420,
                Height = 120
            };
            try
            {
                window.Show();
                window.UpdateLayout();

                Ellipse dot = Assert.IsType<Ellipse>(ElementByName(window, "TitleBarStatusDot"));
                SolidColorBrush fill = Assert.IsType<SolidColorBrush>(dot.Fill);
                SolidColorBrush warning = Assert.IsType<SolidColorBrush>(titleBar.FindResource("StatusWarn"));
                Assert.Equal(warning.Color, fill.Color);
            }
            finally
            {
                window.Close();
            }
        });
    }

    [Fact]
    public void UsesDarkChromeBackgroundResource()
    {
        RunOnSta(() =>
        {
            WpfTestApplication.ApplyTheme(AppTheme.Dark);
            SwitchifyTitleBar titleBar = new()
            {
                TitleText = "Switchify PC dark"
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

                SolidColorBrush background = Assert.IsType<SolidColorBrush>(titleBar.Background);
                Assert.Equal(WpfColor.FromRgb(0xA8, 0x23, 0x1C), background.Color);
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

    private static FrameworkElement? ElementByName(DependencyObject root, string name)
    {
        FrameworkElement? result = null;
        Collect(root, node =>
        {
            if (result is null &&
                node is FrameworkElement element &&
                element.Name == name)
            {
                result = element;
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
