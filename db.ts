import postgres from "https://deno.land/x/postgresjs@v3.2.4/mod.js";
import { z } from "https://deno.land/x/zod@v3.17.3/mod.ts";

const numeric = z.string()
  .transform((val) => parseInt(val, 10))
  .refine((val) => !Number.isNaN(val));

const messageSchema = z.object({
  id: z.number().int(),
  timestamp: z.date(),
  channel: z.string(),
  message: z.string(),
});

export type Message = z.infer<typeof messageSchema>;

const sql = postgres({
  user: "postgres",
  db: "mailbox",

  // Ignore notice messages
  onnotice: () => {},
});

// Setup the database tables
export async function setup() {
  await sql`CREATE TABLE IF NOT EXISTS message (
    id SERIAL PRIMARY KEY,
    timestamp TIMESTAMP DEFAULT now(),
    channel TEXT NOT NULL,
    message TEXT NOT NULL
  );`;
}

// Disconnect from the database
export async function disconnect() {
  await sql.end();
}

// Add a new message to a particular channel, returning the new message
export async function addMessage(
  channel: string,
  message: string,
): Promise<Message> {
  const [newMessage] = await sql
    `INSERT INTO message (channel, message) VALUES (${channel}, ${message}) RETURNING *`;
  return messageSchema.parse(newMessage);
}

// Load the messages in a particular channel, or all messages if the channel is null
export async function loadMessages(channel: string | null): Promise<Message[]> {
  const query = channel === null ? sql`` : sql`WHERE channel = ${channel}`;
  const messages = await sql`SELECT * FROM message ${query}`;
  return messages.map((message) => messageSchema.parse(message));
}

// Delete the messages in a particular channel, or all messages if the channel is null
// Return the deleted messages
export async function clearMessages(
  channel: string | null,
): Promise<Message[]> {
  const query = channel === null ? sql`` : sql`WHERE channel = ${channel}`;
  const messages = await sql`DELETE FROM message ${query} RETURNING *`;
  return messages.map((message) => messageSchema.parse(message));
}

const numMessagesSchema = z.object({
  num_messages: numeric,
});

// Delete the messages in a particular channel, or all messages if the channel is null
export async function countMessages(channel: string | null): Promise<number> {
  const query = channel === null ? sql`` : sql`WHERE channel = ${channel}`;
  const [numMessages] = await sql
    `SELECT count(*) AS num_messages FROM message ${query}`;
  return numMessagesSchema.parse(numMessages).num_messages;
}

const summarizeMessagesSchema = z.array(z.object({
  channel: z.string(),
  num_messages: numeric,
}));

// Count the number of messages in each channel
export async function summarizeMessages(): Promise<Map<string, number>> {
  const totals = await sql
    `SELECT channel, COUNT(*) as num_messages FROM message GROUP BY channel;`;
  return new Map(
    summarizeMessagesSchema.parse(totals).map((
      { channel, num_messages },
    ) => [channel, num_messages]),
  );
}
