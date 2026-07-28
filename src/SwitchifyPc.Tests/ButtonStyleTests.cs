using System.Threading;
using System.Windows;
using System.Windows.Media;
using SwitchifyPc.App;
using SwitchifyPc.Core.SwitchControl;
using WpfBorder = System.Windows.Controls.Border;
using WpfButton = System.Windows.Controls.Button;
using WpfComboBox = System.Windows.Controls.ComboBox;
using WpfControl = System.Windows.Controls.Control;
using WpfControlTemplate = System.Windows.Controls.ControlTemplate;
using WpfListBoxItem = System.Windows.Controls.ListBoxItem;
using WpfSystemColors = System.Windows.SystemColors;
using WpfTextBox = System.Windows.Controls.TextBox;

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

    [Fact]
    public void ProfileWindowPrimaryButtonStyleUsesRedHoverBrush()
    {
        RunOnSta(() =>
        {
            WpfTestApplication.ApplyTheme(SwitchifyPc.App.Themes.AppTheme.Light);
            SwitchControlProfileWindow window = new(new StaticProfileStore(), () => null);
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
    public void ProfileWindowInteractiveStylesUseSystemKeyboardFocusBrush()
    {
        RunOnSta(() =>
        {
            WpfTestApplication.ApplyTheme(SwitchifyPc.App.Themes.AppTheme.Light);
            SwitchControlProfileWindow window = new(new StaticProfileStore(), () => null);
            try
            {
                AssertTemplateFocusBrush(
                    Assert.IsType<Style>(window.Resources[typeof(WpfButton)]),
                    UIElement.IsKeyboardFocusedProperty,
                    WpfBorder.BorderBrushProperty,
                    "Root");
                AssertTemplateFocusBrush(
                    Assert.IsType<Style>(window.FindResource("PrimaryButton")),
                    UIElement.IsKeyboardFocusedProperty,
                    WpfBorder.BorderBrushProperty,
                    "Root");
                AssertTemplateFocusBrush(
                    Assert.IsType<Style>(window.Resources[typeof(WpfListBoxItem)]),
                    UIElement.IsKeyboardFocusedProperty,
                    WpfBorder.BorderBrushProperty,
                    "Root");
                AssertTemplateFocusBrush(
                    Assert.IsType<Style>(window.Resources[typeof(WpfComboBox)]),
                    UIElement.IsKeyboardFocusWithinProperty,
                    WpfBorder.BorderBrushProperty,
                    "DropDownToggle");

                Style textBox = Assert.IsType<Style>(window.Resources[typeof(WpfTextBox)]);
                Trigger textFocus = Assert.IsType<Trigger>(
                    Assert.Single(
                        textBox.Triggers.OfType<Trigger>(),
                        trigger => trigger.Property == UIElement.IsKeyboardFocusWithinProperty));
                Assert.Contains(textFocus.Setters.OfType<Setter>(), setter =>
                    setter.Property == WpfControl.BorderBrushProperty &&
                    IsDynamicResource(setter.Value, WpfSystemColors.HighlightBrushKey));
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

    private static void AssertTemplateFocusBrush(
        Style style,
        DependencyProperty triggerProperty,
        DependencyProperty setterProperty,
        string targetName)
    {
        WpfControlTemplate template = Assert.IsType<WpfControlTemplate>(
            Assert.Single(
                style.Setters.OfType<Setter>(),
                setter => setter.Property == WpfControl.TemplateProperty).Value);
        Trigger focusTrigger = Assert.IsType<Trigger>(
            Assert.Single(
                template.Triggers.OfType<Trigger>(),
                trigger => trigger.Property == triggerProperty));
        Assert.Contains(focusTrigger.Setters.OfType<Setter>(), setter =>
            setter.TargetName == targetName &&
            setter.Property == setterProperty &&
            IsDynamicResource(setter.Value, WpfSystemColors.HighlightBrushKey));
    }

    private static bool IsDynamicResource(object value, object resourceKey)
    {
        return value is DynamicResourceExtension resource &&
            Equals(resource.ResourceKey, resourceKey);
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

    private sealed class StaticProfileStore : ISwitchControlProfileStore
    {
        public IReadOnlyList<SwitchControlProfile> Load() => SwitchControlProfiles.BuiltIns;

        public IReadOnlyList<SwitchControlProfile> Save(IReadOnlyList<SwitchControlProfile> customProfiles) =>
            [.. SwitchControlProfiles.BuiltIns, .. customProfiles];
    }
}
