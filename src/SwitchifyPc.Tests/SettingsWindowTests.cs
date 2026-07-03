using System.Threading;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using SwitchifyPc.App;
using SwitchifyPc.Core.Ui;
using SwitchifyPc.Core.Updates;

namespace SwitchifyPc.Tests;

public sealed class SettingsWindowTests
{
    [Fact]
    public void SettingsWindowLoadsWithUpdateDownloadProgress()
    {
        RunOnSta(() =>
        {
            SettingsViewModel viewModel = new();
            viewModel.SetUpdateState(UpdateState.CreateInitial("0.2.4") with
            {
                Download = new UpdateDownloadProgress(
                    UpdateDownloadStatus.Downloading,
                    20_971_520,
                    52_428_800,
                    40)
            });

            SettingsWindow window = new(viewModel);
            try
            {
                window.Show();
                window.UpdateLayout();
            }
            finally
            {
                window.Close();
            }
        });
    }

    [Fact]
    public void SettingsWindowShowsSeparateMouseRepeatIntervals()
    {
        RunOnSta(() =>
        {
            SettingsViewModel viewModel = new();
            SettingsWindow window = new(viewModel);
            try
            {
                window.Show();
                window.UpdateLayout();

                IReadOnlyList<string> text = TextBlocks(window);
                Assert.Contains("Movement repeat interval", text);
                Assert.Contains("Time between repeated pointer movements.", text);
                Assert.Contains("Scroll repeat interval", text);
                Assert.Contains("Time between repeated scroll actions.", text);
                Assert.DoesNotContain("Repeat Interval", text);
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
        CollectTextBlocks(root, text);
        return text;
    }

    private static void CollectTextBlocks(DependencyObject node, List<string> text)
    {
        if (node is TextBlock textBlock)
        {
            text.Add(textBlock.Text);
        }

        for (int index = 0; index < VisualTreeHelper.GetChildrenCount(node); index++)
        {
            CollectTextBlocks(VisualTreeHelper.GetChild(node, index), text);
        }
    }
}
