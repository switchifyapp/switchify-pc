using System.ComponentModel;
using System.IO;
using System.Runtime.CompilerServices;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Media;
using SwitchifyPc.Core.SwitchControl;
using WpfMessageBox = System.Windows.MessageBox;

namespace SwitchifyPc.App;

public partial class SwitchControlProfileWindow : Window
{
    private readonly ISwitchControlProfileStore store;
    private readonly Func<string?> activeProfileId;
    private readonly Func<MessageBoxResult> confirmUnsavedChanges;
    private IReadOnlyList<SwitchControlProfile> profiles = [];
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
        ProfilesList.ItemsSource = profiles;
        SwitchControlProfile? profile =
            profiles.FirstOrDefault(candidate => candidate.Id == selectId) ?? profiles.FirstOrDefault();
        SelectAndLoad(profile);
    }

    private void ProfilesList_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (suppressSelectionChange ||
            ProfilesList.SelectedItem is not SwitchControlProfile profile ||
            profile.Id == selected?.Id)
        {
            return;
        }

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
        ProfileName.IsEnabled = isEditable;
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
        ProfilesList.ItemsSource = profiles;
        suppressSelectionChange = true;
        ProfilesList.SelectedItem = profile;
        suppressSelectionChange = false;
        LoadProfile(profile, transient: true);
        ProfilesList.ScrollIntoView(profile);
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
        ProfilesList.SelectedItem = profile;
        suppressSelectionChange = false;
        if (profile is not null)
        {
            LoadProfile(profile);
        }
    }

    private void RestoreSelectedProfile()
    {
        suppressSelectionChange = true;
        ProfilesList.SelectedItem = selected;
        suppressSelectionChange = false;
    }

    private void RefreshDirtyState()
    {
        if (suppressDirtyTracking || cleanSnapshot is null) return;
        ProfileEditSnapshot current = CaptureSnapshot();
        isDirty = isEditable &&
            (isTransient ||
             !string.Equals(current.Name, cleanSnapshot.Name, StringComparison.Ordinal) ||
             !current.Bindings.SequenceEqual(cleanSnapshot.Bindings));
        SaveButton.IsEnabled = isDirty;
        CancelButton.IsEnabled = isDirty;
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

    private sealed class BindingRowViewModel : INotifyPropertyChanged
    {
        private SwitchBindingType selectedType;
        private string value = "";
        private bool isEditable;

        public BindingRowViewModel(int switchId)
        {
            SwitchId = switchId;
        }

        public event PropertyChangedEventHandler? PropertyChanged;
        public int SwitchId { get; }
        public string SwitchLabel => $"Switch {SwitchId}";
        public string TypeAutomationName => $"Switch {SwitchId} action type";
        public string ValueAutomationName => $"Switch {SwitchId} action value";
        public IReadOnlyList<SwitchBindingType> Types { get; } = Enum.GetValues<SwitchBindingType>();

        public SwitchBindingType SelectedType
        {
            get => selectedType;
            set
            {
                selectedType = value;
                Changed();
                Changed(nameof(ValueOptions));
                Changed(nameof(ValueHelp));
            }
        }

        public string Value
        {
            get => value;
            set { this.value = value; Changed(); }
        }

        public bool IsEditable
        {
            get => isEditable;
            private set { isEditable = value; Changed(); }
        }

        public IReadOnlyList<string> ValueOptions => SelectedType switch
        {
            SwitchBindingType.Key => KeyValues,
            SwitchBindingType.MouseButton => ["left", "right", "middle"],
            SwitchBindingType.Shortcut => ["Ctrl + C", "Ctrl + V", "Alt + Tab", "Ctrl + Shift + Escape"],
            SwitchBindingType.MouseClick => ["left", "left:2", "right", "right:2", "middle", "middle:2"],
            SwitchBindingType.Scroll => ["up", "down", "left", "right"],
            SwitchBindingType.Media => ["playPause", "nextTrack", "previousTrack", "volumeUp", "volumeDown", "mute"],
            _ => []
        };

        public string ValueHelp => SelectedType switch
        {
            SwitchBindingType.None => "No value is required.",
            SwitchBindingType.Key => "Choose one keyboard key.",
            SwitchBindingType.MouseButton => "Choose left, right, or middle.",
            SwitchBindingType.Shortcut => "Enter one to four unique keys separated by +, including a non-modifier.",
            SwitchBindingType.MouseClick => "Choose a button, with :2 for a double click.",
            SwitchBindingType.Scroll => "Choose up, down, left, or right.",
            SwitchBindingType.Media => "Choose play/pause, track, volume, or mute.",
            _ => "Choose a valid value."
        };

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
            SelectedType = binding.Type;
            Value = binding.Type == SwitchBindingType.Shortcut
                ? string.Join(" + ", binding.Keys ?? [])
                : binding.Type == SwitchBindingType.MouseClick
                    ? $"{binding.Value}:{binding.ClickCount}"
                    : binding.Value ?? "";
            IsEditable = editable;
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
    }
}
