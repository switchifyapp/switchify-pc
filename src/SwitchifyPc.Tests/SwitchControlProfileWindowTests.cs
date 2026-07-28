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
using WpfComboBox = System.Windows.Controls.ComboBox;
using WpfListBox = System.Windows.Controls.ListBox;
using WpfTextBox = System.Windows.Controls.TextBox;

namespace SwitchifyPc.Tests;

[Collection(WpfTestCollection.Name)]
public sealed class SwitchControlProfileWindowTests
{
    [Fact]
    public void ProfileWindowMatchesSettingsChromeAndLayout()
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
                Assert.Equal(ResizeMode.CanResize, window.ResizeMode);
                Assert.Equal(900, window.Width);
                Assert.Equal(690, window.Height);
                Assert.Equal(520, window.MinWidth);
                Assert.Equal(300, window.MinHeight);
                Assert.NotNull(WindowChrome.GetWindowChrome(window));
                Assert.Contains("PC Switch Control profiles", TextBlocks(window));
                Assert.NotNull(ButtonByAutomationName(window, "Minimize"));
                Assert.NotNull(ButtonByAutomationName(window, "Maximize"));
                Assert.NotNull(ButtonByAutomationName(window, "Close"));

                WpfButton save = Assert.IsType<WpfButton>(window.FindName("SaveButton"));
                Assert.Same(window.FindResource("PrimaryButton"), save.Style);
                Assert.DoesNotContain("Close", ButtonContent(window));
            }
            finally
            {
                window.Close();
            }
        });
    }

    [Fact]
    public void ProfileWindowClampsToWorkAreaAndUsesCompactLayout()
    {
        RunOnSta(() =>
        {
            WpfTestApplication.ApplyTheme(AppTheme.Light);
            SwitchControlProfileWindow window = CreateWindow();
            try
            {
                window.Show();
                window.ApplyWorkArea(new Rect(100, 50, 680, 420));
                window.UpdateLayout();

                Assert.Equal(648, window.Width);
                Assert.Equal(388, window.Height);
                Assert.Equal(116, window.Left);
                Assert.Equal(66, window.Top);
                Assert.Equal(Visibility.Collapsed, Assert.IsType<StackPanel>(window.FindName("IntroPanel")).Visibility);

                Border profiles = Assert.IsType<Border>(window.FindName("ProfilesPanel"));
                Border editor = Assert.IsType<Border>(window.FindName("EditorPanel"));
                Grid footer = Assert.IsType<Grid>(window.FindName("FooterPanel"));
                Assert.Equal(0, Grid.GetRow(profiles));
                Assert.Equal(1, Grid.GetRow(editor));
                Assert.Equal(2, Grid.GetRow(footer));
                Assert.Equal(0, Grid.GetColumn(editor));
                Assert.Equal(1, Grid.GetColumnSpan(footer));
            }
            finally
            {
                window.Close();
            }
        });
    }

    [Fact]
    public void ProfileWindowEnablesEditActionsOnlyWhileDirty()
    {
        RunOnSta(() =>
        {
            MutableProfileStore store = new();
            SwitchControlProfileWindow window = CreateWindow(store, () => MessageBoxResult.No);
            try
            {
                window.Show();
                window.UpdateLayout();
                SelectCustomProfile(window);

                WpfButton save = Assert.IsType<WpfButton>(window.FindName("SaveButton"));
                WpfButton cancel = Assert.IsType<WpfButton>(window.FindName("CancelButton"));
                WpfTextBox name = Assert.IsType<WpfTextBox>(window.FindName("ProfileName"));
                Assert.False(save.IsEnabled);
                Assert.False(cancel.IsEnabled);

                name.Text = "Edited profile";
                Assert.True(save.IsEnabled);
                Assert.True(cancel.IsEnabled);

                name.Text = "Custom profile";
                Assert.False(save.IsEnabled);
                Assert.False(cancel.IsEnabled);

                WpfComboBox action = Assert.IsType<WpfComboBox>(
                    ControlByAutomationName<WpfComboBox>(window, "Switch 1 action type"));
                WpfComboBox value = Assert.IsType<WpfComboBox>(
                    ControlByAutomationName<WpfComboBox>(window, "Switch 1 action value"));
                action.SelectedItem = SwitchBindingType.None;
                Assert.True(save.IsEnabled);
                action.SelectedItem = SwitchBindingType.Key;
                value.Text = "A";
                Assert.False(save.IsEnabled);

                value.Text = "B";
                Assert.True(save.IsEnabled);
                value.Text = "A";
                Assert.False(save.IsEnabled);
            }
            finally
            {
                window.Close();
            }
        });
    }

    [Fact]
    public void NewAndDuplicateProfilesBeginDirty()
    {
        RunOnSta(() =>
        {
            MutableProfileStore store = new();
            SwitchControlProfileWindow window = CreateWindow(store, () => MessageBoxResult.No);
            try
            {
                window.Show();
                window.UpdateLayout();

                WpfButton save = Assert.IsType<WpfButton>(window.FindName("SaveButton"));
                WpfButton cancel = Assert.IsType<WpfButton>(window.FindName("CancelButton"));
                ButtonByContent(window, "New").RaiseEvent(new RoutedEventArgs(WpfButton.ClickEvent));
                Assert.True(save.IsEnabled);
                Assert.True(cancel.IsEnabled);

                cancel.RaiseEvent(new RoutedEventArgs(WpfButton.ClickEvent));
                SelectCustomProfile(window);
                ButtonByContent(window, "Duplicate").RaiseEvent(new RoutedEventArgs(WpfButton.ClickEvent));
                Assert.True(save.IsEnabled);
                Assert.True(cancel.IsEnabled);
            }
            finally
            {
                window.Close();
            }
        });
    }

    [Theory]
    [InlineData(MessageBoxResult.Yes, "Edited profile", "Grid 3")]
    [InlineData(MessageBoxResult.No, "Custom profile", "Grid 3")]
    [InlineData(MessageBoxResult.Cancel, "Custom profile", "Custom profile")]
    public void ProfileSelectionResolvesUnsavedChanges(
        MessageBoxResult decision,
        string storedName,
        string selectedName)
    {
        RunOnSta(() =>
        {
            MessageBoxResult promptDecision = decision;
            MutableProfileStore store = new();
            SwitchControlProfileWindow window = CreateWindow(store, () => promptDecision);
            try
            {
                window.Show();
                window.UpdateLayout();
                SelectCustomProfile(window);
                Assert.IsType<WpfTextBox>(window.FindName("ProfileName")).Text = "Edited profile";

                WpfListBox profiles = Assert.IsType<WpfListBox>(window.FindName("ProfilesList"));
                profiles.SelectedItem = profiles.Items
                    .Cast<SwitchControlProfile>()
                    .First(profile => profile.IsBuiltIn);

                Assert.Equal(selectedName, Assert.IsType<SwitchControlProfile>(profiles.SelectedItem).Name);
                Assert.Equal(storedName, store.CustomProfiles.Single().Name);
            }
            finally
            {
                promptDecision = MessageBoxResult.No;
                window.Close();
            }
        });
    }

    [Theory]
    [InlineData(MessageBoxResult.Yes, false, "Edited profile")]
    [InlineData(MessageBoxResult.No, false, "Custom profile")]
    [InlineData(MessageBoxResult.Cancel, true, "Custom profile")]
    public void ProfileWindowClosingResolvesUnsavedChanges(
        MessageBoxResult decision,
        bool remainsOpen,
        string storedName)
    {
        RunOnSta(() =>
        {
            MessageBoxResult promptDecision = decision;
            MutableProfileStore store = new();
            SwitchControlProfileWindow window = CreateWindow(store, () => promptDecision);
            window.Show();
            window.UpdateLayout();
            SelectCustomProfile(window);
            Assert.IsType<WpfTextBox>(window.FindName("ProfileName")).Text = "Edited profile";

            window.Close();

            Assert.Equal(remainsOpen, window.IsVisible);
            Assert.Equal(storedName, store.CustomProfiles.Single().Name);
            if (window.IsVisible)
            {
                promptDecision = MessageBoxResult.No;
                window.Close();
            }
        });
    }

    [Fact]
    public void InvalidProfilePreventsSelectionAndClosingWhenSaving()
    {
        RunOnSta(() =>
        {
            MessageBoxResult promptDecision = MessageBoxResult.Yes;
            MutableProfileStore store = new();
            SwitchControlProfileWindow window = CreateWindow(store, () => promptDecision);
            try
            {
                window.Show();
                window.UpdateLayout();
                SelectCustomProfile(window);

                WpfComboBox action = Assert.IsType<WpfComboBox>(
                    ControlByAutomationName<WpfComboBox>(window, "Switch 1 action type"));
                WpfComboBox value = Assert.IsType<WpfComboBox>(
                    ControlByAutomationName<WpfComboBox>(window, "Switch 1 action value"));
                action.SelectedItem = SwitchBindingType.Key;
                value.Text = "NotAKey";

                WpfListBox profiles = Assert.IsType<WpfListBox>(window.FindName("ProfilesList"));
                profiles.SelectedItem = profiles.Items
                    .Cast<SwitchControlProfile>()
                    .First(profile => profile.IsBuiltIn);

                Assert.Equal("Custom profile", Assert.IsType<SwitchControlProfile>(profiles.SelectedItem).Name);
                Assert.Contains(
                    "Switch 1",
                    Assert.IsType<TextBlock>(window.FindName("ValidationMessage")).Text);

                window.Close();
                Assert.True(window.IsVisible);
            }
            finally
            {
                promptDecision = MessageBoxResult.No;
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

    private static SwitchControlProfileWindow CreateWindow(
        MutableProfileStore store,
        Func<MessageBoxResult> confirmUnsavedChanges) =>
        new(store, () => null, confirmUnsavedChanges);

    private static void SelectCustomProfile(SwitchControlProfileWindow window)
    {
        WpfListBox profiles = Assert.IsType<WpfListBox>(window.FindName("ProfilesList"));
        profiles.SelectedItem = profiles.Items
            .Cast<SwitchControlProfile>()
            .Single(profile => !profile.IsBuiltIn);
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

    private static WpfButton? ButtonByAutomationName(DependencyObject root, string name)
        => ControlByAutomationName<WpfButton>(root, name);

    private static T? ControlByAutomationName<T>(DependencyObject root, string name)
        where T : DependencyObject
    {
        T? result = null;
        Collect(root, node =>
        {
            if (result is null &&
                node is T control &&
                AutomationProperties.GetName(control) == name)
            {
                result = control;
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

    private static WpfButton ButtonByContent(DependencyObject root, string content)
    {
        WpfButton? result = null;
        Collect(root, node =>
        {
            if (result is null && node is WpfButton { Content: string text } button && text == content)
            {
                result = button;
            }
        });
        return Assert.IsType<WpfButton>(result);
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

    private sealed class MutableProfileStore : ISwitchControlProfileStore
    {
        public MutableProfileStore()
        {
            CustomProfiles =
            [
                new SwitchControlProfile(
                    "custom-profile",
                    1,
                    "Custom profile",
                    SwitchControlProviderKind.Mapped,
                    Enumerable.Range(1, 8)
                        .Select(id => id == 1
                            ? new SwitchControlBinding(id, SwitchBindingType.Key, "A")
                            : new SwitchControlBinding(id, SwitchBindingType.None))
                        .ToArray())
            ];
        }

        public IReadOnlyList<SwitchControlProfile> CustomProfiles { get; private set; }

        public IReadOnlyList<SwitchControlProfile> Load() =>
            [.. SwitchControlProfiles.BuiltIns, .. CustomProfiles];

        public IReadOnlyList<SwitchControlProfile> Save(IReadOnlyList<SwitchControlProfile> customProfiles)
        {
            CustomProfiles = customProfiles;
            return Load();
        }
    }
}
