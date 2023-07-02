# http_server

The mailbox `http_server` package is an HTTP server written in [Actix](https://actix.rs/) and deployed with [Shuttle](https://shuttle.rs/). It connects to a Postgres mailbox database and allows you to interact with the mailbox through a REST API instead of a CLI, which is preferable in some situations. For example, suppose you have a script that runs hourly in a Cloudflare Worker that scrapes a webpage for new data. That script can create messages based simply by making calls to a REST endpoint.

## REST API

All responses are in JSON.

### Filters

All API endpoints support filtering the messages that will be interacted with via query string parameters. The available filters are as follows:

- `ids`: comma-separated list of message ids
  - `?ids=3` matches the message with the id `3`
  - `?ids=1,4,5` matches the messages with the ids `1`, `4`, `5`
- `mailbox`: name of the mailbox or parent mailbox
  - `?mailbox=my-script` matches all messages in `my-script` and all children mailboxes like `my-script/mailbox-1` and `my-script/mailbox-2`
  - `?mailbox=my-script/mailbox-1` matches all messages in `my-script/mailbox-1` and all children mailboxes like `my-script/mailbox-1/sub-mailbox` and `my-script/mailbox-2/sub-mailbox`
- `states`: comma-separated list of message states (`unread`, `read`, or `archived`)
  - `?states=read` matches all read messages
  - `?states=unread,archived` matches all unread or archived messages

Filters can also be combined. For example, `?mailbox=other-script&states=read,archived` matches read or archived messages in the `other-script` mailbox.

If no filter is provided, all messages will be interacted with.

### Authorization

All requests must be sent with an `Authorization` header of `Bearer {API_TOKEN}` where `{API_TOKEN}` is your configured API token (see [Running locally](#running-locally) for information on configuring the API token).

### Message format

All responses are JSON arrays of message objects. The format of message objects is as follows:

- `id` (integer): the message's id
- `timestamp` (string): the message's creation date in UTC ISO format
- `mailbox` (string): the message's mailbox
- `content` (string): the message's content
- `state` (string): the message's state, which will be one of `unread`, `read`, or `archived`

Example message:

```json
{
  "id": 123,
  "timestamp": "2023-01-01T12:01:02.345678",
  "mailbox": "foo",
  "content": "bar",
  "state": "unread"
}
```

### `GET /api/messages`

Reads messages. Responds with a JSON array of messages matching the optional message filter.

### `GET /api/mailboxes`

Reads mailbox sizes. Responds with a JSON object where the key is the mailbox name and the value is the number of messages that the mailbox contains. If an optional message filter is provided, only messages that match the filter are counted towards mailbox sizes.

Example response:

```json
{
  "mailbox-1": 5,
  "mailbox-2": 2,
  "mailbox-3": 4
}
```

### `POST /api/messages`

Creates new messages. Responds with a JSON array of the created messages. This is the only endpoint that does not accept a message filter. New messages should be posted as JSON in the request body with a `Content-Type` header of `application/json`. The body can either be a single message object or an array of message objects. These are the accepted message object fields:

- `mailbox` (string): the message's mailbox
- `content` (string): the message's content
- `state` (string optional): the message's state, which will be one of `unread`, `read`, or `archived` (defaults to `unread` if omitted)

Example single-message payload:

```json
{
  "mailbox": "my-script",
  "content": "Hello, world!",
  "state": "read"
}
```

Example multi-message payload:

```json
[
  {
    "mailbox": "my-script",
    "content": "Hello, world!"
  },
  {
    "mailbox": "my-script",
    "content": "Hello, universe!",
    "state": "archived"
  }
]
```

### `PUT /api/messages`

Updates message states. Responds with a JSON array of the updated messages. Only updates messages matching the optional filter. If no filter is provided, all messages are updated. The new message state should be put as a JSON encoded object with a single field `new_state` in the request body with a `Content-Type` header of `application/json`. `new_state` can have the value `unread`, `read`, or `archived`.

Example request payload to mark all messages as read:

```
{"new_state": "read"}
```

### `DELETE /api/messages`

Permanently deletes messages. Responds with a JSON array of the deleted messages. Only updates messages matching the optional filter. Unlike the other endpoints, if no filter is provided, an error is returned instead of deleting all messages as a safety measure to prevent data loss.

## Running locally

```sh
# Install Shuttle
$ cargo install cargo-shuttle

# Copy secrets definitions
$ cp http_server/Secrets.example.toml http_server/Secrets.dev.toml

# Manually edit http_server/Secrets.dev.toml if desired

# Run the server
$ cargo shuttle run --working-directory http_server
```

To change the database used, modify the `DATABASE_URL` secret in `http_server/Secrets.dev.toml` to point to a local or remote Postgres database. To change the expected API token, modify the `API_TOKEN` secret to be a long randomly-generated string of your own choosing.
