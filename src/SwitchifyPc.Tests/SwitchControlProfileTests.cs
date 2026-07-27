using SwitchifyPc.Core.SwitchControl;

namespace SwitchifyPc.Tests;

public sealed class SwitchControlProfileTests
{
    [Fact]
    public void BuiltInsHaveStableIdentityAndMappings()
    {
        SwitchControlProfile grid = SwitchControlProfiles.BuiltIns[0];
        SwitchControlProfile keyboard = SwitchControlProfiles.BuiltIns[1];

        Assert.Equal("builtin.grid3", grid.Id);
        Assert.Equal("Grid 3", grid.Name);
        Assert.Equal(1, grid.Version);
        Assert.Equal("builtin.keyboard", keyboard.Id);
        Assert.Equal("Generic keyboard", keyboard.Name);
        Assert.Equal(SwitchBindingType.Key, keyboard.Bindings[0].Type);
        Assert.Equal("Space", keyboard.Bindings[0].Value);
        Assert.Equal("Enter", keyboard.Bindings[1].Value);
        Assert.All(keyboard.Bindings.Skip(2), binding => Assert.Equal(SwitchBindingType.None, binding.Type));
    }

    [Fact]
    public void StoreRoundTripsValidatedCustomProfiles()
    {
        string path = Path.Combine(Path.GetTempPath(), $"switch-control-{Guid.NewGuid():N}.json");
        try
        {
            var store = new JsonSwitchControlProfileStore(path);
            SwitchControlProfile custom = CustomProfile("My profile");

            IReadOnlyList<SwitchControlProfile> saved = store.Save([custom]);
            IReadOnlyList<SwitchControlProfile> loaded = store.Load();

            Assert.Equal(3, saved.Count);
            Assert.Equal(custom.Id, loaded[2].Id);
            Assert.Equal(custom.Name, loaded[2].Name);
            Assert.Equal(custom.Version, loaded[2].Version);
            Assert.Equal(custom.Bindings, loaded[2].Bindings);
        }
        finally
        {
            File.Delete(path);
        }
    }

    [Fact]
    public void StorePreservesMalformedFileAndLoadsBuiltIns()
    {
        string path = Path.Combine(Path.GetTempPath(), $"switch-control-{Guid.NewGuid():N}.json");
        File.WriteAllText(path, "{broken");
        try
        {
            var warnings = new List<string>();
            var store = new JsonSwitchControlProfileStore(path, warnings.Add);

            IReadOnlyList<SwitchControlProfile> loaded = store.Load();

            Assert.Equal(2, loaded.Count);
            Assert.Equal("{broken", File.ReadAllText(path));
            Assert.Single(warnings);
        }
        finally
        {
            File.Delete(path);
        }
    }

    [Fact]
    public void DuplicateNamesAreRejectedCaseInsensitively()
    {
        SwitchControlProfile first = CustomProfile("Writing");
        SwitchControlProfile second = CustomProfile(" writing ");

        Assert.Throws<InvalidDataException>(() =>
            JsonSwitchControlProfileStore.NormalizeCustom([first, second]));
    }

    [Fact]
    public void ShortcutRequiresUniqueKeysAndNonModifier()
    {
        SwitchControlProfile invalid = CustomProfile(
            "Shortcut",
            new(1, SwitchBindingType.Shortcut, Keys: ["Ctrl", "Alt"]));

        Assert.Throws<InvalidDataException>(() =>
            JsonSwitchControlProfileStore.NormalizeCustom([invalid]));
    }

    [Fact]
    public void KeyTokensAreCanonicalizedBeforePersistence()
    {
        SwitchControlProfile profile = CustomProfile(
            "Canonical",
            new(1, SwitchBindingType.Shortcut, Keys: ["ctrl", "a"]));

        SwitchControlProfile normalized =
            Assert.Single(JsonSwitchControlProfileStore.NormalizeCustom([profile]));

        Assert.Equal(["Ctrl", "A"], normalized.Bindings[0].Keys);
    }

    private static SwitchControlProfile CustomProfile(
        string name,
        SwitchControlBinding? firstBinding = null) =>
        new(
            Guid.NewGuid().ToString(),
            1,
            name,
            SwitchControlProviderKind.Mapped,
            [
                firstBinding ?? new(1, SwitchBindingType.Key, "Space"),
                .. Enumerable.Range(2, 7).Select(id => new SwitchControlBinding(id, SwitchBindingType.None))
            ]);
}
