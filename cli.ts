import Denomander from "https://deno.land/x/denomander@0.9.3/mod.ts";
import {
  addMessage,
  clearMessages,
  countMessages,
  loadMessages,
  Message,
  setup,
  summarizeMessages,
} from "./db.ts";

function formatMessage(message: Message): string {
  return `${message.message} [${message.channel}] @ ${message.timestamp.toISOString()}`;
}

const program = new Denomander({
  app_name: "Mailbox",
  app_description: "Message manager for local commands",
  app_version: "0.1.0",
});

program.command("add [channel] [message]", "Add a message to a channel")
  .action(async (args: { channel: string; message: string }) => {
    console.log(formatMessage(await addMessage(args.channel, args.message)));
  });

program.command("clear [channel?]", "Clear the messages")
  .action(async (args: { channel: string | undefined }) => {
    const messages = await clearMessages(args.channel ?? null);
    console.log(messages.map((message) => formatMessage(message)).join("\n"));
  });

program.command("count [channel?]", "Count the messages")
  .action(async (args: { channel: string | undefined }) => {
    console.log(await countMessages(args.channel ?? null));
  });

program.command("summary", "Summarize the messages")
  .action(async () => {
    const summary = (await summarizeMessages());
    console.log(
      Array.from(summary.entries()).map(([channel, count]) =>
        `[${channel}]: ${count}`
      ).join("\n"),
    );
  });

program.command("view [channel?]", "View the messages")
  .action(async (args: { channel: string | undefined }) => {
    const messages = (await loadMessages(args.channel ?? null));
    console.log(messages.map((message) => formatMessage(message)).join("\n"));
  });

await setup();

program.parse(Deno.args);
