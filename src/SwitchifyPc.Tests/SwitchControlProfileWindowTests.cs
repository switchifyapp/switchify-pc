using System.Threading;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Input;
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
                Assert.InRange(window.Width, window.MinWidth, 900);
                Assert.InRange(window.Height, window.MinHeight, 690);
                Assert.True(window.Width <= Math.Max(320, SystemParameters.WorkArea.Width - 32));
                Assert.True(window.Height <= Math.Max(240, SystemParameters.WorkArea.Height - 32));
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
                action.SelectedValue = SwitchBindingType.None;
                Assert.True(save.IsEnabled);
                action.SelectedValue = SwitchBindingType.Key;
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
                    .Cast<object>()
                    .First(item => ProfileForItem(item).IsBuiltIn);

                object? selectedItem = profiles.SelectedItem;
                Assert.NotNull(selectedItem);
                Assert.Equal(selectedName, ProfileForItem(selectedItem).Name);
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
                action.SelectedValue = SwitchBindingType.Key;
                value.Text = "NotAKey";

                WpfListBox profiles = Assert.IsType<WpfListBox>(window.FindName("ProfilesList"));
                profiles.SelectedItem = profiles.Items
                    .Cast<object>()
                    .First(item => ProfileForItem(item).IsBuiltIn);

                object? selectedItem = profiles.SelectedItem;
                Assert.NotNull(selectedItem);
                Assert.Equal("Custom profile", ProfileForItem(selectedItem).Name);
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
    public void ProfileWindowUsesFriendlyLabelsAndSavesCanonicalValues()
    {
        RunOnSta(() =>
        {
            WpfTestApplication.ApplyTheme(AppTheme.Light);
            MutableProfileStore store = new();
            SwitchControlProfileWindow window = CreateWindow(store, () => MessageBoxResult.No);
            try
            {
                window.Show();
                window.UpdateLayout();
                SelectCustomProfile(window);

                WpfComboBox action = Assert.IsType<WpfComboBox>(
                    ControlByAutomationName<WpfComboBox>(window, "Switch 1 action type"));
                WpfComboBox value = Assert.IsType<WpfComboBox>(
                    ControlByAutomationName<WpfComboBox>(window, "Switch 1 action value"));
                WpfButton save = Assert.IsType<WpfButton>(window.FindName("SaveButton"));

                Assert.Contains("Mouse button", ItemLabels(action));
                Assert.Contains("Media control", ItemLabels(action));
                action.SelectedValue = SwitchBindingType.MouseClick;
                Assert.Empty(value.Text);
                Assert.False(save.IsEnabled);
                Assert.Contains("Choose a value for this action.", TextBlocks(window));
                Assert.Contains("Double left click", ItemLabels(value));

                value.Text = "Double left click";
                Assert.True(save.IsEnabled);
                save.RaiseEvent(new RoutedEventArgs(WpfButton.ClickEvent));

                SwitchControlBinding saved = store.CustomProfiles.Single().Bindings[0];
                Assert.Equal(SwitchBindingType.MouseClick, saved.Type);
                Assert.Equal("left", saved.Value);
                Assert.Equal(2, saved.ClickCount);
            }
            finally
            {
                window.Close();
            }
        });
    }

    [Fact]
    public void ChangingEveryActionTypeClearsThePreviousValue()
    {
        RunOnSta(() =>
        {
            WpfTestApplication.ApplyTheme(AppTheme.Light);
            MutableProfileStore store = new();
            SwitchControlProfileWindow window = CreateWindow(store, () => MessageBoxResult.No);
            try
            {
                window.Show();
                window.UpdateLayout();
                SelectCustomProfile(window);

                WpfComboBox action = Assert.IsType<WpfComboBox>(
                    ControlByAutomationName<WpfComboBox>(window, "Switch 1 action type"));
                WpfComboBox value = Assert.IsType<WpfComboBox>(
                    ControlByAutomationName<WpfComboBox>(window, "Switch 1 action value"));
                var displays = new Dictionary<SwitchBindingType, string>
                {
                    [SwitchBindingType.Key] = "Up arrow",
                    [SwitchBindingType.MouseButton] = "Left button",
                    [SwitchBindingType.Shortcut] = "Ctrl + C",
                    [SwitchBindingType.MouseClick] = "Left click",
                    [SwitchBindingType.Scroll] = "Scroll down",
                    [SwitchBindingType.Media] = "Play / pause"
                };

                action.SelectedValue = SwitchBindingType.None;
                Assert.Empty(value.Text);
                foreach ((SwitchBindingType type, string display) in displays)
                {
                    action.SelectedValue = type;
                    Assert.Empty(value.Text);
                    value.Text = display;
                    Assert.Equal(display, value.Text);
                }
            }
            finally
            {
                window.Close();
            }
        });
    }

    [Theory]
    [InlineData(SwitchBindingType.Key, "Up arrow", "Key|ArrowUp|1|")]
    [InlineData(SwitchBindingType.MouseButton, "Left button", "MouseButton|left|1|")]
    [InlineData(SwitchBindingType.Shortcut, "Windows key + A", "Shortcut||1|Meta,A")]
    [InlineData(SwitchBindingType.MouseClick, "Double right click", "MouseClick|right|2|")]
    [InlineData(SwitchBindingType.Scroll, "Scroll up", "Scroll|up|1|")]
    [InlineData(SwitchBindingType.Media, "Play / pause", "Media|playPause|1|")]
    public void FriendlyValuesRoundTripToCanonicalBindings(
        SwitchBindingType type,
        string display,
        string expected)
    {
        RunOnSta(() =>
        {
            WpfTestApplication.ApplyTheme(AppTheme.Light);
            MutableProfileStore store = new();
            SwitchControlProfileWindow window = CreateWindow(store, () => MessageBoxResult.No);
            try
            {
                window.Show();
                window.UpdateLayout();
                SelectCustomProfile(window);

                WpfComboBox action = Assert.IsType<WpfComboBox>(
                    ControlByAutomationName<WpfComboBox>(window, "Switch 1 action type"));
                WpfComboBox value = Assert.IsType<WpfComboBox>(
                    ControlByAutomationName<WpfComboBox>(window, "Switch 1 action value"));
                action.SelectedValue = type;
                value.Text = display;
                Assert.True(Assert.IsType<WpfButton>(window.FindName("SaveButton")).IsEnabled);
                Assert.IsType<WpfButton>(window.FindName("SaveButton"))
                    .RaiseEvent(new RoutedEventArgs(WpfButton.ClickEvent));

                SwitchControlBinding binding = store.CustomProfiles.Single().Bindings[0];
                Assert.Equal(
                    expected,
                    $"{binding.Type}|{binding.Value}|{binding.ClickCount}|{string.Join(",", binding.Keys ?? [])}");
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

    [Fact]
    public void BuiltInAndActiveProfilesExposeBadgesAndFocusableReadOnlyValues()
    {
        RunOnSta(() =>
        {
            WpfTestApplication.ApplyTheme(AppTheme.Light);
            SwitchControlProfileWindow window = new(
                new StaticProfileStore(),
                () => SwitchControlProfiles.Grid3Id);
            try
            {
                window.Show();
                window.UpdateLayout();

                Assert.Equal(
                    2,
                    ControlsByAutomationName<Border>(window, "Built-in profile").Count(border => border.IsVisible));
                Assert.Single(
                    ControlsByAutomationName<Border>(window, "Active profile"),
                    border => border.IsVisible);

                WpfTextBox profileName = Assert.IsType<WpfTextBox>(window.FindName("ProfileName"));
                Assert.True(profileName.IsEnabled);
                Assert.True(profileName.IsReadOnly);
                Assert.True(profileName.IsTabStop);

                WpfComboBox hiddenEditor = Assert.IsType<WpfComboBox>(
                    ControlByAutomationName<WpfComboBox>(window, "Switch 1 action type"));
                Assert.Equal(Visibility.Collapsed, hiddenEditor.Visibility);
                WpfTextBox readOnlyAction = Assert.IsType<WpfTextBox>(
                    ControlByAutomationName<WpfTextBox>(window, "Switch 1 action type"));
                WpfTextBox readOnlyValue = Assert.IsType<WpfTextBox>(
                    ControlByAutomationName<WpfTextBox>(window, "Switch 1 action value"));
                Assert.Equal(Visibility.Visible, readOnlyAction.Visibility);
                Assert.True(readOnlyAction.IsReadOnly);
                Assert.True(readOnlyAction.IsTabStop);
                Assert.True(readOnlyValue.IsReadOnly);
                Assert.True(readOnlyValue.IsTabStop);
                Assert.Equal("No value", readOnlyValue.Text);
            }
            finally
            {
                window.Close();
            }
        });
    }

    [Fact]
    public void ProfileActionsExposeAccessKeysAndShortcutHelp()
    {
        RunOnSta(() =>
        {
            WpfTestApplication.ApplyTheme(AppTheme.Light);
            SwitchControlProfileWindow window = CreateWindow();
            try
            {
                window.Show();
                window.UpdateLayout();

                AssertAccessKey(window, "New profile", "_New", "Alt+N");
                AssertAccessKey(window, "Duplicate profile", "D_uplicate", "Alt+U");
                AssertAccessKey(window, "Delete profile", "_Delete", "Alt+D");
                AssertAccessKey(window, "Cancel changes", "_Cancel changes", "Alt+C");
                AssertAccessKey(window, "Save profile", "_Save", "Ctrl+S or Alt+S");
            }
            finally
            {
                window.Close();
            }
        });
    }

    [Fact]
    public void ControlSavesAndEscapeClosesWithoutMakingEnterAWindowShortcut()
    {
        RunOnSta(() =>
        {
            WpfTestApplication.ApplyTheme(AppTheme.Light);
            MutableProfileStore store = new();
            SwitchControlProfileWindow window = CreateWindow(store, () => MessageBoxResult.No);
            window.Show();
            window.UpdateLayout();
            SelectCustomProfile(window);
            Assert.IsType<WpfTextBox>(window.FindName("ProfileName")).Text = "Keyboard profile";

            Assert.True(window.HandleKeyboardShortcut(Key.S, ModifierKeys.Control));
            Assert.Equal("Keyboard profile", store.CustomProfiles.Single().Name);
            Assert.False(window.HandleKeyboardShortcut(Key.Enter, ModifierKeys.None));
            Assert.True(window.IsVisible);
            Assert.True(window.HandleKeyboardShortcut(Key.Escape, ModifierKeys.None));
            Assert.False(window.IsVisible);
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
            .Cast<object>()
            .Single(item => !ProfileForItem(item).IsBuiltIn);
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

    private static IReadOnlyList<T> ControlsByAutomationName<T>(DependencyObject root, string name)
        where T : DependencyObject
    {
        List<T> results = [];
        Collect(root, node =>
        {
            if (node is T control && AutomationProperties.GetName(control) == name)
            {
                results.Add(control);
            }
        });
        return results;
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

    private static IReadOnlyList<string> ItemLabels(ItemsControl items) =>
        items.Items.Cast<object>()
            .Select(item => item.GetType().GetProperty("Label")?.GetValue(item) as string)
            .Where(label => label is not null)
            .Cast<string>()
            .ToArray();

    private static void AssertAccessKey(
        DependencyObject root,
        string automationName,
        string content,
        string helpText)
    {
        WpfButton button = Assert.IsType<WpfButton>(ButtonByAutomationName(root, automationName));
        Assert.Equal(content, Assert.IsType<string>(button.Content));
        Assert.Equal(helpText, AutomationProperties.GetHelpText(button));
    }

    private static WpfButton ButtonByContent(DependencyObject root, string content)
    {
        WpfButton? result = null;
        Collect(root, node =>
        {
            if (result is null &&
                node is WpfButton { Content: string text } button &&
                text.Replace("_", "", StringComparison.Ordinal) == content)
            {
                result = button;
            }
        });
        return Assert.IsType<WpfButton>(result);
    }

    private static SwitchControlProfile ProfileForItem(object item) =>
        Assert.IsType<SwitchControlProfile>(
            item.GetType().GetProperty("Profile")?.GetValue(item));

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
