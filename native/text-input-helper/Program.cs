using System.Text.Json;

namespace Switchify.TextInput;

internal static class Program
{
    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        PropertyNameCaseInsensitive = true
    };

    private static async Task Main()
    {
        Console.Out.WriteLine("""{"type":"ready"}""");
        Console.Out.Flush();

        while (true)
        {
            string? line = await Console.In.ReadLineAsync();
            if (line is null)
            {
                return;
            }

            if (!HandleCommandLine(line))
            {
                return;
            }
        }
    }

    private static bool HandleCommandLine(string line)
    {
        TextInputCommand? command;
        try
        {
            command = JsonSerializer.Deserialize<TextInputCommand>(line, JsonOptions);
        }
        catch (JsonException error)
        {
            WriteError(null, "invalid_command", error.Message);
            return true;
        }

        if (command?.Type is null)
        {
            WriteError(command?.Id, "invalid_command", "Command type is required.");
            return true;
        }

        switch (command.Type)
        {
            case "typeText":
                HandleTypeText(command);
                return true;
            case "shutdown":
                return false;
            default:
                WriteError(command.Id, "invalid_command", "Unsupported command type.");
                return true;
        }
    }

    private static void HandleTypeText(TextInputCommand command)
    {
        if (string.IsNullOrWhiteSpace(command.Id))
        {
            WriteError(null, "invalid_command", "Command id is required.");
            return;
        }

        if (command.Text is null)
        {
            WriteError(command.Id, "invalid_command", "Command text is required.");
            return;
        }

        try
        {
            int sentEvents = NativeMethods.SendUnicodeText(command.Text);
            Console.Out.WriteLine(JsonSerializer.Serialize(new
            {
                type = "result",
                id = command.Id,
                ok = true,
                sentEvents
            }));
            Console.Out.Flush();
        }
        catch (Exception error)
        {
            WriteError(command.Id, "send_input_failed", error.Message);
        }
    }

    private static void WriteError(string? id, string code, string message)
    {
        Console.Out.WriteLine(JsonSerializer.Serialize(new
        {
            type = "error",
            id,
            ok = false,
            code,
            message
        }));
        Console.Out.Flush();
    }
}
