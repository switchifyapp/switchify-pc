using System.Threading;
using System.Windows;
using System.Windows.Media;
using SwitchifyPc.App;
using WpfBorder = System.Windows.Controls.Border;
using WpfControl = System.Windows.Controls.Control;
using WpfControlTemplate = System.Windows.Controls.ControlTemplate;

namespace SwitchifyPc.Tests;

[Collection(WpfTestCollection.Name)]
public sealed class ButtonStyleTests
{
    [Fact]
    public void MainWindowPrimaryButtonStyleUsesRedHoverBrush()
    {
        RunOnSta(() =>
        {
            WpfTestApplication.ApplyTheme(SwitchifyPc.App.Themes.AppTheme.Light);
            MainWindow window = new();
            try
            {
                AssertPrimaryButtonStyleUsesRedHoverBrush(window);
            }
            finally
            {
                window.Close();
            }
        });
    }

    [Fact]
    public void SettingsWindowPrimaryButtonStyleUsesRedHoverBrush()
    {
        RunOnSta(() =>
        {
            WpfTestApplication.ApplyTheme(SwitchifyPc.App.Themes.AppTheme.Light);
            SettingsWindow window = new();
            try
            {
                AssertPrimaryButtonStyleUsesRedHoverBrush(window);
            }
            finally
            {
                window.Close();
            }
        });
    }

    private static void AssertPrimaryButtonStyleUsesRedHoverBrush(FrameworkElement element)
    {
        Style style = Assert.IsType<Style>(element.FindResource("PrimaryButton"));
        Assert.Null(style.BasedOn);

        WpfControlTemplate template = Assert.IsType<WpfControlTemplate>(
            Assert.Single(style.Setters.OfType<Setter>(), setter => setter.Property == WpfControl.TemplateProperty).Value);
        Trigger hoverTrigger = Assert.IsType<Trigger>(
            Assert.Single(template.Triggers.OfType<Trigger>(), trigger => trigger.Property == UIElement.IsMouseOverProperty));

        Assert.Contains(hoverTrigger.Setters.OfType<Setter>(), setter =>
            setter.TargetName == "Root" &&
            setter.Property == WpfBorder.BackgroundProperty &&
            IsDynamicResource(setter.Value, "BrandPrimaryHover"));
        Assert.Contains(hoverTrigger.Setters.OfType<Setter>(), setter =>
            setter.TargetName == "Root" &&
            setter.Property == WpfBorder.BorderBrushProperty &&
            IsDynamicResource(setter.Value, "BrandPrimaryHover"));
    }

    private static bool IsDynamicResource(object value, string resourceKey)
    {
        return value is DynamicResourceExtension resource &&
            string.Equals(resource.ResourceKey?.ToString(), resourceKey, StringComparison.Ordinal);
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
}
