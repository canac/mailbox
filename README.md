# mailbox

`mailbox` is a CLI tool that scripts and other commands can use to asynchronously deliver messages to the user. All messages are created locally by local commands and stored locally. It is most useful for use in scripts that run in the background on a schedule. For example, a daily online backup script could create messages for the status of the backup. Or a web scraper could create messages when it finds information of interest.

## Adding a message

The first step is creating a new message.

```sh
$ mailbox add my-script "Hello, world!"
* Hello, world! [my-script] @ 2022-03-14 05:09:25
```

Messages are organized into mailboxes. A mailbox is simply a collection of messages. The first argument to `mailbox add` is the mailbox name. The second argument is the message content. The output shows that a new message was created. The mailbox name is between the square brackets, and the timestamp is after the @ sign. The asterisk (\*) at the beginning is an indicator that the message hasn't been read yet.

## Reading messages

As messages are created in the background, the next step is to read them. There are a couple of options. `mailbox view` shows all unread messages.

```sh
$ mailbox view
* Hello, world! [my-script] @ 2022-03-14 05:09:25
```

`mailbox read` shows all unread messages _and_ marks them as read. It also displays all the messages that were read.

```sh
$ mailbox read
  Hello, world! [my-script] @ 2022-03-14 05:09:25

$ mailbox view

```

Notice that the unread asterisk is gone and `mailbox view` no longer shows the message because it is unread.

`mailbox archive` archives all messages, whether read or unread, and marks them as read. It also displays all the messages that were archived.

```sh
$ mailbox archive
- Hello, world! [my-script] @ 2022-03-14 05:09:25
```

The dash (-) is an indicator that the message is now archived.

Messages can be in one of three states:

- Unread (\* indicator)
- Read (no indicator)
- Archived (- indicator)

`mailbox view` only shows unread messages by default, but read and archived messages can be viewed by passing the `--state` flag.

```sh
$ mailbox view --state=archived
- Hello, world! [my-script] @ 2022-03-14 05:09:25
```

The possible values for state are:

- `unread` (only unread messages, default)
- `read` (only read messages)
- `archived` (only archived messages)
- `unarchived` (only unread and read messages)
- `all` (all messages, regardless of state)

Messages can also be filtered by their mailbox with the `--mailbox` flag. `view`, `read`, `archive`, and `clear` all accept the `--mailbox` flag. When messages are categorized into sub mailboxes separated by a `/` (`first-script/errors`, for example), the `--mailbox` filter include the parent mailbox and any sub mailboxes.

```sh
$ mailbox add first-script "Hello, world!"
* Hello, world! [first-script] @ 2022-03-14 05:09:25

$ mailbox add second-script "Hello, universe!"
* Hello, world! [second-script] @ 2022-03-14 05:09:25

$ mailbox add second-script/errors "Whoops!"
* Hello, world! [second-script/errors] @ 2022-03-14 05:09:25

$ mailbox view
* Hello, world! [first-script] @ 2022-03-14 05:09:25
* Hello, world! [second-script] @ 2022-03-14 05:09:25
* Whoops! [second-script/errors] @ 2022-03-14 05:09:25

$ mailbox view --mailbox=second-script
* Hello, world! [second-script] @ 2022-03-14 05:09:25
* Whoops! [second-script/errors] @ 2022-03-14 05:09:25

$ mailbox view --mailbox=second-script/errors
* Whoops! [second-script/errors] @ 2022-03-14 05:09:25
```

## Clearing messages

The final stage of a message's lifecycle is being deleted. `mailbox clear` permanently clears all archived messages.

```sh
$ mailbox clear
- Hello, world! [my-script] @ 2022-03-14 05:09:25

$ mailbox view --state=all

```

## Typical workflow

A typical workflow when using mailbox is to first check for any new messages by running `mailbox view`. Then, if there aren't any messages that you want to continue to be reminded about, run `mailbox read`. Alternatively, when you don't want to see any of those messages again, run `mailbox archive`. Periodically, optionally run `mailbox clear` to prevent archived messages from building up.
