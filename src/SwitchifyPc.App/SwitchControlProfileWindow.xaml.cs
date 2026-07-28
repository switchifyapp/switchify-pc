using System.ComponentModel;
using System.IO;
using System.Runtime.CompilerServices;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using SwitchifyPc.Core.SwitchControl;
using WpfKeyEventArgs = System.Windows.Input.KeyEventArgs;
using WpfMessageBox = System.Windows.MessageBox;

namespace SwitchifyPc.App;

public partial class SwitchControlProfileWindow : Window
{
    private readonly ISwitchControlProfileStore store;
    private readonly Func<string?> activeProfileId;
    private readonly Func<MessageBoxResult> confirmUnsavedChanges;
    private IReadOnlyList<SwitchControlProfile> profiles = [];
    private IReadOnlyList<ProfileListItem> profileItems = [];
    private SwitchControlProfile? selected;
    private ProfileEditSnapshot? cleanSnapshot;
    private bool isEditable;
    private bool isTransient;
    private bool isDirty;
    private bool suppressDirtyTracking;
    private bool suppressSelectionChange;
    private bool isCompactLayout;
    private readonly BindingRowViewModel[] rows =
        Enumerable.Range(1, 8).Select(id => new BindingRowViewModel(id)).ToArray();

    public SwitchControlProfileWindow(
        ISwitchControlProfileStore store,
        Func<string?> activeProfileId)
        : this(store, activeProfileId, ShowUnsavedChangesPrompt)
    {
    }

    internal SwitchControlProfileWindow(
        ISwitchControlProfileStore store,
        Func<string?> activeProfileId,
        Func<MessageBoxResult> confirmUnsavedChanges)
    {
        this.store = store;
        this.activeProfileId = activeProfileId;
        this.confirmUnsavedChanges = confirmUnsavedChanges;
        InitializeComponent();
        BindingRows.ItemsSource = rows;
        ProfileName.TextChanged += (_, _) => RefreshDirtyState();
        foreach (BindingRowViewModel row in rows)
        {
            row.PropertyChanged += (_, _) => RefreshDirtyState();
        }
        Loaded += (_, _) => ApplyWorkArea(SystemParameters.WorkArea);
        Reload();
    }

    internal void ApplyWorkArea(Rect workArea)
    {
        const double workAreaMargin = 16;
        double availableWidth = Math.Max(320, workArea.Width - workAreaMargin * 2);
        double availableHeight = Math.Max(240, workArea.Height - workAreaMargin * 2);

        MinWidth = Math.Min(520, availableWidth);
        MinHeight = Math.Min(300, availableHeight);
        Width = Math.Min(900, availableWidth);
        Height = Math.Min(690, availableHeight);
        Left = workArea.Left + Math.Max(0, (workArea.Width - Width) / 2);
        Top = workArea.Top + Math.Max(0, (workArea.Height - Height) / 2);
        ApplyResponsiveLayout(Width);
    }

    private void Window_SizeChanged(object sender, SizeChangedEventArgs e)
    {
        ApplyResponsiveLayout(e.NewSize.Width);
    }

    private void ApplyResponsiveLayout(double width)
    {
        bool useCompactLayout = width < 720;
        if (useCompactLayout == isCompactLayout)
        {
            return;
        }

        isCompactLayout = useCompactLayout;
        IntroPanel.Visibility = useCompactLayout ? Visibility.Collapsed : Visibility.Visible;
        ContentBackground.Margin = useCompactLayout
            ? new Thickness(0)
            : new Thickness(0, 80, 0, 0);
        ProfileBody.Margin = useCompactLayout
            ? new Thickness(16, 12, 16, 16)
            : new Thickness(24, 0, 24, 24);
        ProfilesPanel.Margin = useCompactLayout
            ? new Thickness(0, 0, 0, 12)
            : new Thickness(0);

        ProfilesColumn.Width = useCompactLayout
            ? new GridLength(1, GridUnitType.Star)
            : new GridLength(230);
        ContentGapColumn.Width = useCompactLayout ? new GridLength(0) : new GridLength(18);
        EditorColumn.Width = useCompactLayout ? new GridLength(0) : new GridLength(1, GridUnitType.Star);
        PrimaryContentRow.Height = useCompactLayout
            ? new GridLength(92)
            : new GridLength(1, GridUnitType.Star);
        SecondaryContentRow.Height = useCompactLayout
            ? new GridLength(1, GridUnitType.Star)
            : new GridLength(0);

        Grid.SetRow(ProfilesPanel, 0);
        Grid.SetColumn(ProfilesPanel, 0);
        Grid.SetColumnSpan(ProfilesPanel, 1);
        Grid.SetRow(EditorPanel, useCompactLayout ? 1 : 0);
        Grid.SetColumn(EditorPanel, useCompactLayout ? 0 : 2);
        Grid.SetColumnSpan(EditorPanel, 1);
        Grid.SetRow(FooterPanel, 2);
        Grid.SetColumn(FooterPanel, 0);
        Grid.SetColumnSpan(FooterPanel, useCompactLayout ? 1 : 3);
    }

    private void Reload(string? selectId = null)
    {
        profiles = store.Load();
        RefreshProfileItems();
        SwitchControlProfile? profile =
            profiles.FirstOrDefault(candidate => candidate.Id == selectId) ?? profiles.FirstOrDefault();
        SelectAndLoad(profile);
    }

    private void RefreshProfileItems()
    {
        string? activeId = activeProfileId();
        profileItems = profiles
            .Select(profile => new ProfileListItem(profile, profile.Id == activeId))
            .ToArray();
        ProfilesList.ItemsSource = profileItems;
    }

    private void ProfilesList_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (suppressSelectionChange ||
            ProfilesList.SelectedItem is not ProfileListItem item ||
            item.Profile.Id == selected?.Id)
        {
            return;
        }

        SwitchControlProfile profile = item.Profile;
        if (isDirty)
        {
            string targetId = profile.Id;
            switch (confirmUnsavedChanges())
            {
                case MessageBoxResult.Yes:
                    if (!TrySave())
                    {
                        RestoreSelectedProfile();
                        return;
                    }
                    Reload(targetId);
                    return;
                case MessageBoxResult.No:
                    Reload(targetId);
                    return;
                default:
                    RestoreSelectedProfile();
                    return;
            }
        }

        LoadProfile(profile);
    }

    protected override void OnClosing(CancelEventArgs e)
    {
        if (isDirty)
        {
            MessageBoxResult decision = confirmUnsavedChanges();
            if (decision == MessageBoxResult.Cancel ||
                decision == MessageBoxResult.Yes && !TrySave())
            {
                e.Cancel = true;
            }
        }

        base.OnClosing(e);
    }

    private void LoadProfile(SwitchControlProfile profile, bool transient = false)
    {
        suppressDirtyTracking = true;
        selected = profile;
        isEditable = !profile.IsBuiltIn && profile.Id != activeProfileId();
        isTransient = transient;
        ProfileName.Text = profile.Name;
        ProfileName.IsEnabled = true;
        ProfileName.IsReadOnly = !isEditable;
        ProfileName.IsTabStop = true;
        ReadOnlyMessage.Text = profile.IsBuiltIn
            ? "Built-in profiles are read-only. Duplicate this profile to customize it."
            : isEditable
                ? ""
                : "This profile is active and cannot be changed until PC Switch Control stops.";
        foreach ((BindingRowViewModel row, SwitchControlBinding binding) in rows.Zip(profile.Bindings))
        {
            row.Load(binding, isEditable);
        }
        cleanSnapshot = CaptureSnapshot();
        suppressDirtyTracking = false;
        DeleteButton.IsEnabled = isEditable;
        ValidationMessage.Text = "";
        RefreshDirtyState();
    }

    private void New_Click(object sender, RoutedEventArgs e)
    {
        if (!ResolvePendingChanges()) return;
        EditUnsaved(new SwitchControlProfile(
            Guid.NewGuid().ToString(),
            1,
            UniqueName("New profile"),
            SwitchControlProviderKind.Mapped,
            Enumerable.Range(1, 8).Select(id => new SwitchControlBinding(id, SwitchBindingType.None)).ToArray()));
    }

    private void Duplicate_Click(object sender, RoutedEventArgs e)
    {
        if (!ResolvePendingChanges()) return;
        if (selected is null) return;
        EditUnsaved(selected with
        {
            Id = Guid.NewGuid().ToString(),
            Version = 1,
            Name = UniqueName($"{selected.Name} copy"),
            IsBuiltIn = false,
            Kind = SwitchControlProviderKind.Mapped,
            Bindings = selected.Kind == SwitchControlProviderKind.Grid3
                ? Enumerable.Range(1, 8).Select(id => new SwitchControlBinding(id, SwitchBindingType.None)).ToArray()
                : selected.Bindings.Select(binding => binding with { }).ToArray()
        });
    }

    private void EditUnsaved(SwitchControlProfile profile)
    {
        profiles = [.. profiles, profile];
        RefreshProfileItems();
        ProfileListItem item = profileItems.Single(candidate => candidate.Profile.Id == profile.Id);
        suppressSelectionChange = true;
        ProfilesList.SelectedItem = item;
        suppressSelectionChange = false;
        LoadProfile(profile, transient: true);
        ProfilesList.ScrollIntoView(item);
    }

    private void Save_Click(object sender, RoutedEventArgs e) => TrySave();

    private bool TrySave()
    {
        if (selected is null || !isEditable) return false;
        BindingRowViewModel? invalidRow = rows.FirstOrDefault(row => !row.IsLocallyValid());
        if (invalidRow is not null)
        {
            ValidationMessage.Text = $"Switch {invalidRow.SwitchId}: {invalidRow.ValueHelp}";
            FocusBindingValue(invalidRow);
            return false;
        }
        try
        {
            SwitchControlProfile saved = selected with
            {
                Name = ProfileName.Text,
                Version = store.Load().Any(profile => profile.Id == selected.Id)
                    ? selected.Version + 1
                    : 1,
                Bindings = rows.Select(row => row.ToBinding()).ToArray()
            };
            IReadOnlyList<SwitchControlProfile> custom = profiles
                .Where(profile => !profile.IsBuiltIn && profile.Id != selected.Id)
                .Append(saved)
                .ToArray();
            store.Save(custom);
            Reload(saved.Id);
            return true;
        }
        catch (Exception error) when (error is InvalidDataException or IOException or UnauthorizedAccessException)
        {
            ValidationMessage.Text = error.Message;
            ProfileName.Focus();
            return false;
        }
    }

    private void Cancel_Click(object sender, RoutedEventArgs e) => Reload(selected?.Id);

    private void Delete_Click(object sender, RoutedEventArgs e)
    {
        if (selected is null || selected.IsBuiltIn || selected.Id == activeProfileId()) return;
        MessageBoxResult answer = WpfMessageBox.Show(
            $"Delete “{selected.Name}”?",
            "Delete PC Switch Control profile",
            MessageBoxButton.YesNo,
            MessageBoxImage.Warning);
        if (answer != MessageBoxResult.Yes) return;
        store.Save(profiles.Where(profile => !profile.IsBuiltIn && profile.Id != selected.Id).ToArray());
        Reload();
    }

    private bool ResolvePendingChanges()
    {
        if (!isDirty) return true;
        return confirmUnsavedChanges() switch
        {
            MessageBoxResult.Yes => TrySave(),
            MessageBoxResult.No => DiscardPendingChanges(),
            _ => false
        };
    }

    private bool DiscardPendingChanges()
    {
        Reload(selected?.Id);
        return true;
    }

    private void SelectAndLoad(SwitchControlProfile? profile)
    {
        suppressSelectionChange = true;
        ProfilesList.SelectedItem = profile is null
            ? null
            : profileItems.FirstOrDefault(item => item.Profile.Id == profile.Id);
        suppressSelectionChange = false;
        if (profile is not null)
        {
            LoadProfile(profile);
        }
    }

    private void RestoreSelectedProfile()
    {
        suppressSelectionChange = true;
        ProfilesList.SelectedItem = selected is null
            ? null
            : profileItems.FirstOrDefault(item => item.Profile.Id == selected.Id);
        suppressSelectionChange = false;
    }

    private void Window_PreviewKeyDown(object sender, WpfKeyEventArgs e)
    {
        e.Handled = HandleKeyboardShortcut(
            e.Key,
            Keyboard.Modifiers,
            Keyboard.FocusedElement is System.Windows.Controls.ComboBox { IsDropDownOpen: true });
    }

    internal bool HandleKeyboardShortcut(
        Key key,
        ModifierKeys modifiers,
        bool isComboBoxOpen = false)
    {
        if (key == Key.S && modifiers == ModifierKeys.Control && SaveButton.IsEnabled)
        {
            return TrySave();
        }

        if (key == Key.Escape && modifiers == ModifierKeys.None && !isComboBoxOpen)
        {
            Close();
            return true;
        }

        return false;
    }

    private void RefreshDirtyState()
    {
        if (suppressDirtyTracking || cleanSnapshot is null) return;
        ProfileEditSnapshot current = CaptureSnapshot();
        isDirty = isEditable &&
            (isTransient ||
             !string.Equals(current.Name, cleanSnapshot.Name, StringComparison.Ordinal) ||
             !current.Bindings.SequenceEqual(cleanSnapshot.Bindings));
        bool isValid = IsProfileLocallyValid();
        SaveButton.IsEnabled = isDirty && isValid;
        CancelButton.IsEnabled = isDirty;
        ValidationMessage.Text = IsProfileNameLocallyValid()
            ? ""
            : "Profile names must be unique and contain 1 to 50 characters.";
    }

    private bool IsProfileLocallyValid() =>
        IsProfileNameLocallyValid() && rows.All(row => row.IsLocallyValid());

    private bool IsProfileNameLocallyValid()
    {
        string name = ProfileName.Text.Trim();
        return name.Length is >= 1 and <= 50 &&
            profiles.All(profile =>
                profile.Id == selected?.Id ||
                !string.Equals(profile.Name, name, StringComparison.OrdinalIgnoreCase));
    }

    private ProfileEditSnapshot CaptureSnapshot() =>
        new(
            ProfileName.Text,
            rows.Select(row => new BindingEditSnapshot(row.SelectedType, row.Value)).ToArray());

    private static MessageBoxResult ShowUnsavedChangesPrompt() =>
        WpfMessageBox.Show(
            "Save changes to this profile before continuing?\n\nChoose No to discard them.",
            "Unsaved PC Switch Control changes",
            MessageBoxButton.YesNoCancel,
            MessageBoxImage.Warning);

    private string UniqueName(string proposed)
    {
        string name = proposed;
        int suffix = 2;
        while (profiles.Any(profile => string.Equals(profile.Name, name, StringComparison.OrdinalIgnoreCase)))
        {
            name = $"{proposed} {suffix++}";
        }
        return name;
    }

    private void FocusBindingValue(BindingRowViewModel row)
    {
        BindingRows.UpdateLayout();
        if (BindingRows.ItemContainerGenerator.ContainerFromItem(row) is not DependencyObject container)
        {
            return;
        }
        FindDescendants<System.Windows.Controls.ComboBox>(container)
            .FirstOrDefault(control =>
                AutomationProperties.GetName(control) == row.ValueAutomationName)
            ?.Focus();
    }

    private static IEnumerable<T> FindDescendants<T>(DependencyObject parent)
        where T : DependencyObject
    {
        for (int index = 0; index < VisualTreeHelper.GetChildrenCount(parent); index++)
        {
            DependencyObject child = VisualTreeHelper.GetChild(parent, index);
            if (child is T match) yield return match;
            foreach (T descendant in FindDescendants<T>(child)) yield return descendant;
        }
    }

    private sealed record ProfileEditSnapshot(
        string Name,
        IReadOnlyList<BindingEditSnapshot> Bindings);

    private sealed record BindingEditSnapshot(
        SwitchBindingType Type,
        string Value);

    private sealed record ProfileListItem(
        SwitchControlProfile Profile,
        bool IsActive)
    {
        public string Name => Profile.Name;
        public bool IsBuiltIn => Profile.IsBuiltIn;
        public string AccessibleName => IsBuiltIn && IsActive
            ? $"{Name}, built-in, active"
            : IsBuiltIn
                ? $"{Name}, built-in"
                : IsActive
                    ? $"{Name}, active"
                    : Name;
    }

    private sealed record BindingTypeOption(
        SwitchBindingType Value,
        string Label)
    {
        public override string ToString() => Label;
    }

    private sealed record BindingValueOption(
        string Value,
        string Label)
    {
        public override string ToString() => Label;
    }

    private sealed class BindingRowViewModel : INotifyPropertyChanged
    {
        private SwitchBindingType selectedType;
        private string value = "";
        private bool isEditable;
        private bool isLoading;

        public BindingRowViewModel(int switchId)
        {
            SwitchId = switchId;
        }

        public event PropertyChangedEventHandler? PropertyChanged;
        public int SwitchId { get; }
        public string SwitchLabel => $"Switch {SwitchId}";
        public string TypeAutomationName => $"Switch {SwitchId} action type";
        public string ValueAutomationName => $"Switch {SwitchId} action value";
        public IReadOnlyList<BindingTypeOption> Types { get; } = BindingTypes;
        public string SelectedTypeLabel =>
            BindingTypes.First(option => option.Value == SelectedType).Label;

        public SwitchBindingType SelectedType
        {
            get => selectedType;
            set
            {
                if (selectedType == value)
                {
                    return;
                }

                selectedType = value;
                if (!isLoading)
                {
                    SetRawValue("");
                }
                Changed();
                Changed(nameof(SelectedTypeLabel));
                Changed(nameof(ValueOptions));
                Changed(nameof(ValueHelp));
                Changed(nameof(FeedbackText));
                Changed(nameof(HasValidationError));
                Changed(nameof(ReadOnlyValueDisplay));
            }
        }

        public string Value
        {
            get => value;
            set => SetRawValue(value);
        }

        public string ValueDisplay
        {
            get => DisplayValue(SelectedType, value);
            set => SetRawValue(RawValue(SelectedType, value));
        }

        public string ReadOnlyValueDisplay =>
            SelectedType == SwitchBindingType.None || string.IsNullOrWhiteSpace(ValueDisplay)
                ? "No value"
                : ValueDisplay;

        public bool IsEditable
        {
            get => isEditable;
            private set
            {
                isEditable = value;
                Changed();
                Changed(nameof(IsReadOnly));
                Changed(nameof(FeedbackText));
                Changed(nameof(HasValidationError));
            }
        }

        public bool IsReadOnly => !IsEditable;

        public IReadOnlyList<BindingValueOption> ValueOptions => SelectedType switch
        {
            SwitchBindingType.Key => KeyOptions,
            SwitchBindingType.MouseButton => MouseButtonOptions,
            SwitchBindingType.Shortcut => ShortcutOptions,
            SwitchBindingType.MouseClick => MouseClickOptions,
            SwitchBindingType.Scroll => ScrollOptions,
            SwitchBindingType.Media => MediaOptions,
            _ => []
        };

        public string ValueHelp => SelectedType switch
        {
            SwitchBindingType.None => "No value is required.",
            SwitchBindingType.Key => "Choose one keyboard key.",
            SwitchBindingType.MouseButton => "Choose a mouse button to hold while the switch is pressed.",
            SwitchBindingType.Shortcut => "Enter one to four unique keys separated by +, including a non-modifier.",
            SwitchBindingType.MouseClick => "Choose a single or double mouse click.",
            SwitchBindingType.Scroll => "Choose a scroll direction.",
            SwitchBindingType.Media => "Choose play/pause, track, volume, or mute.",
            _ => "Choose a valid value."
        };

        public bool HasValidationError => IsEditable && !IsLocallyValid();

        public string FeedbackText => HasValidationError
            ? string.IsNullOrWhiteSpace(Value)
                ? "Choose a value for this action."
                : $"The value is not valid. {ValueHelp}"
            : ValueHelp;

        public bool IsLocallyValid()
        {
            string trimmed = Value.Trim();
            return SelectedType switch
            {
                SwitchBindingType.None => true,
                SwitchBindingType.Key => KeyValues.Contains(trimmed, StringComparer.OrdinalIgnoreCase),
                SwitchBindingType.MouseButton =>
                    new[] { "left", "right", "middle" }.Contains(trimmed, StringComparer.Ordinal),
                SwitchBindingType.Shortcut => IsValidShortcut(trimmed),
                SwitchBindingType.MouseClick =>
                    new[] { "left", "left:2", "right", "right:2", "middle", "middle:2" }
                        .Contains(trimmed, StringComparer.Ordinal),
                SwitchBindingType.Scroll =>
                    new[] { "up", "down", "left", "right" }.Contains(trimmed, StringComparer.Ordinal),
                SwitchBindingType.Media =>
                    new[] { "playPause", "nextTrack", "previousTrack", "volumeUp", "volumeDown", "mute" }
                        .Contains(trimmed, StringComparer.Ordinal),
                _ => false
            };
        }

        public void Load(SwitchControlBinding binding, bool editable)
        {
            isLoading = true;
            try
            {
                SelectedType = binding.Type;
                Value = binding.Type == SwitchBindingType.Shortcut
                    ? string.Join(" + ", binding.Keys ?? [])
                    : binding.Type == SwitchBindingType.MouseClick
                        ? $"{binding.Value}:{binding.ClickCount}"
                        : binding.Value ?? "";
                IsEditable = editable;
            }
            finally
            {
                isLoading = false;
            }
        }

        public SwitchControlBinding ToBinding()
        {
            string trimmed = Value.Trim();
            if (SelectedType == SwitchBindingType.Shortcut)
            {
                return new(SwitchId, SelectedType, Keys: trimmed.Split(
                    '+',
                    StringSplitOptions.TrimEntries | StringSplitOptions.RemoveEmptyEntries));
            }
            if (SelectedType == SwitchBindingType.MouseClick)
            {
                string[] parts = trimmed.Split(':', StringSplitOptions.TrimEntries);
                int clicks = parts.Length == 2 && int.TryParse(parts[1], out int parsed) ? parsed : 1;
                return new(SwitchId, SelectedType, parts[0], ClickCount: clicks);
            }
            return new(SwitchId, SelectedType, string.IsNullOrEmpty(trimmed) ? null : trimmed);
        }

        private void SetRawValue(string newValue)
        {
            if (string.Equals(value, newValue, StringComparison.Ordinal))
            {
                return;
            }

            value = newValue;
            Changed(nameof(Value));
            Changed(nameof(ValueDisplay));
            Changed(nameof(ReadOnlyValueDisplay));
            Changed(nameof(FeedbackText));
            Changed(nameof(HasValidationError));
        }

        private void Changed([CallerMemberName] string? propertyName = null) =>
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(propertyName));

        private static bool IsValidShortcut(string value)
        {
            string[] keys = value.Split(
                '+',
                StringSplitOptions.TrimEntries | StringSplitOptions.RemoveEmptyEntries);
            return keys.Length is >= 1 and <= 4 &&
                keys.Distinct(StringComparer.OrdinalIgnoreCase).Count() == keys.Length &&
                keys.All(key => KeyValues.Contains(key, StringComparer.OrdinalIgnoreCase)) &&
                keys.Any(key => !ModifierValues.Contains(key, StringComparer.OrdinalIgnoreCase));
        }

        private static string DisplayValue(SwitchBindingType type, string rawValue)
        {
            if (type == SwitchBindingType.Shortcut)
            {
                return string.Join(
                    " + ",
                    rawValue.Split('+', StringSplitOptions.TrimEntries | StringSplitOptions.RemoveEmptyEntries)
                        .Select(key => OptionLabel(KeyOptions, key)));
            }

            return OptionLabel(OptionsFor(type), rawValue);
        }

        private static string RawValue(SwitchBindingType type, string displayValue)
        {
            if (type == SwitchBindingType.Shortcut)
            {
                return string.Join(
                    " + ",
                    displayValue.Split('+', StringSplitOptions.TrimEntries | StringSplitOptions.RemoveEmptyEntries)
                        .Select(key => OptionValue(KeyOptions, key)));
            }

            return OptionValue(OptionsFor(type), displayValue);
        }

        private static IReadOnlyList<BindingValueOption> OptionsFor(SwitchBindingType type) => type switch
        {
            SwitchBindingType.Key => KeyOptions,
            SwitchBindingType.MouseButton => MouseButtonOptions,
            SwitchBindingType.Shortcut => ShortcutOptions,
            SwitchBindingType.MouseClick => MouseClickOptions,
            SwitchBindingType.Scroll => ScrollOptions,
            SwitchBindingType.Media => MediaOptions,
            _ => []
        };

        private static string OptionLabel(IReadOnlyList<BindingValueOption> options, string value) =>
            options.FirstOrDefault(option =>
                string.Equals(option.Value, value, StringComparison.OrdinalIgnoreCase))?.Label ?? value;

        private static string OptionValue(IReadOnlyList<BindingValueOption> options, string display) =>
            options.FirstOrDefault(option =>
                string.Equals(option.Label, display, StringComparison.OrdinalIgnoreCase) ||
                string.Equals(option.Value, display, StringComparison.OrdinalIgnoreCase))?.Value ?? display.Trim();

        private static string KeyLabel(string value) => value switch
        {
            "ArrowUp" => "Up arrow",
            "ArrowDown" => "Down arrow",
            "ArrowLeft" => "Left arrow",
            "ArrowRight" => "Right arrow",
            "PageUp" => "Page up",
            "PageDown" => "Page down",
            "Meta" => "Windows key",
            _ => value
        };

        private static readonly BindingTypeOption[] BindingTypes =
        [
            new(SwitchBindingType.None, "Unassigned"),
            new(SwitchBindingType.Key, "Keyboard key"),
            new(SwitchBindingType.MouseButton, "Mouse button"),
            new(SwitchBindingType.Shortcut, "Keyboard shortcut"),
            new(SwitchBindingType.MouseClick, "Mouse click"),
            new(SwitchBindingType.Scroll, "Scroll"),
            new(SwitchBindingType.Media, "Media control")
        ];
        private static readonly string[] ModifierValues = ["Ctrl", "Alt", "Shift", "Meta"];
        private static readonly string[] KeyValues =
        [
            .. Enumerable.Range('A', 26).Select(value => ((char)value).ToString()),
            .. Enumerable.Range(0, 10).Select(value => value.ToString()),
            .. Enumerable.Range(1, 12).Select(value => $"F{value}"),
            "Space", "Enter", "Escape", "Tab", "Backspace", "Delete",
            "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight",
            "Home", "End", "PageUp", "PageDown", "Ctrl", "Alt", "Shift", "Meta"
        ];
        private static readonly BindingValueOption[] KeyOptions =
            KeyValues.Select(value => new BindingValueOption(value, KeyLabel(value))).ToArray();
        private static readonly BindingValueOption[] MouseButtonOptions =
        [
            new("left", "Left button"),
            new("right", "Right button"),
            new("middle", "Middle button")
        ];
        private static readonly BindingValueOption[] ShortcutOptions =
        [
            new("Ctrl + C", "Ctrl + C"),
            new("Ctrl + V", "Ctrl + V"),
            new("Alt + Tab", "Alt + Tab"),
            new("Ctrl + Shift + Escape", "Ctrl + Shift + Escape")
        ];
        private static readonly BindingValueOption[] MouseClickOptions =
        [
            new("left:1", "Left click"),
            new("left:2", "Double left click"),
            new("right:1", "Right click"),
            new("right:2", "Double right click"),
            new("middle:1", "Middle click"),
            new("middle:2", "Double middle click")
        ];
        private static readonly BindingValueOption[] ScrollOptions =
        [
            new("up", "Scroll up"),
            new("down", "Scroll down"),
            new("left", "Scroll left"),
            new("right", "Scroll right")
        ];
        private static readonly BindingValueOption[] MediaOptions =
        [
            new("playPause", "Play / pause"),
            new("nextTrack", "Next track"),
            new("previousTrack", "Previous track"),
            new("volumeUp", "Volume up"),
            new("volumeDown", "Volume down"),
            new("mute", "Mute")
        ];
    }
}
