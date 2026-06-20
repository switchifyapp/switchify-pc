using System.Text.Json;

namespace Switchify.CursorOverlay;

internal static class Program
{
    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        PropertyNameCaseInsensitive = true
    };

    [STAThread]
    private static void Main()
    {
        Application.SetHighDpiMode(HighDpiMode.PerMonitorV2);
        ApplicationConfiguration.Initialize();
        SynchronizationContext uiContext = SynchronizationContext.Current ?? new WindowsFormsSynchronizationContext();

        using OverlayForm overlayForm = new();
        using CancellationTokenSource cancellation = new();

        Task.Run(() => ReadCommands(overlayForm, uiContext, cancellation.Token));
        Console.Out.WriteLine("""{"type":"ready"}""");
        Console.Out.Flush();

        Application.Run();
        cancellation.Cancel();
    }

    private static async Task ReadCommands(
        OverlayForm overlayForm,
        SynchronizationContext uiContext,
        CancellationToken cancellationToken)
    {
        try
        {
            while (!cancellationToken.IsCancellationRequested)
            {
                string? line = await Console.In.ReadLineAsync(cancellationToken);
                if (line is null)
                {
                    BeginExit(uiContext);
                    return;
                }

                HandleCommandLine(overlayForm, uiContext, line);
            }
        }
        catch (OperationCanceledException)
        {
        }
        catch (Exception error)
        {
            WriteError("internal_error", error.Message);
            BeginExit(uiContext);
        }
    }

    private static void HandleCommandLine(OverlayForm overlayForm, SynchronizationContext uiContext, string line)
    {
        OverlayCommand? command;
        try
        {
            command = JsonSerializer.Deserialize<OverlayCommand>(line, JsonOptions);
        }
        catch (JsonException error)
        {
            WriteError("invalid_command", error.Message);
            return;
        }

        if (command?.Type is null)
        {
            WriteError("invalid_command", "Command type is required.");
            return;
        }

        switch (command.Type)
        {
            case "show":
                if (command.Event is not ("move" or "click" or "drag"))
                {
                    WriteError("invalid_command", "Show command event must be move, click, or drag.");
                    return;
                }

                uiContext.Post(_ => overlayForm.ShowOverlay(command), null);
                break;
            case "hide":
                uiContext.Post(_ => overlayForm.HideOverlay(), null);
                break;
            case "shutdown":
                BeginExit(uiContext);
                break;
            default:
                WriteError("invalid_command", "Unsupported command type.");
                break;
        }
    }

    private static void BeginExit(SynchronizationContext uiContext)
    {
        uiContext.Post(_ => Application.Exit(), null);
    }

    private static void WriteError(string code, string message)
    {
        Console.Out.WriteLine(JsonSerializer.Serialize(new
        {
            type = "error",
            code,
            message
        }));
        Console.Out.Flush();
    }
}
