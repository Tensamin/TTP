import { log } from "@tensamin/shared/log";
import { z } from "zod";

import { RESPONSE_TIMEOUT } from "./values";

export const READY_STATE = {
  CONNECTING: 0,
  OPEN: 1,
  CLOSING: 2,
  CLOSED: 3,
} as const;

const CLOSE_FRAME_LEN = 0xffff_ffff;
const APPLICATION_CLOSE_CODE = 0;
const APPLICATION_CLOSE_REASON = "epsilon-close";
const MAX_REQUEST_ID = 0xffff_fffe;

const textEncoder = new TextEncoder();
const textDecoder = new TextDecoder();

type PrimitiveDataKind = "bool" | "number" | "string" | "container" | "null";
type DataKind = PrimitiveDataKind | { array: PrimitiveDataKind | "container" | "null" };

type WebTransportCloseOptions = {
  closeCode?: number;
  reason?: string;
};

type WebTransportLike = {
  ready: Promise<unknown>;
  closed: Promise<unknown>;
  createUnidirectionalStream(): Promise<WritableStream<Uint8Array>>;
  incomingUnidirectionalStreams: ReadableStream<ReadableStream<Uint8Array>>;
  close(options?: WebTransportCloseOptions): void;
};

type WebTransportGlobal = typeof globalThis & {
  WebTransport?: new (url: string) => WebTransportLike;
};

type PendingRequest = {
  requestType: string;
  resolve: (value: TypedMessage) => void;
  reject: (reason: unknown) => void;
  timeoutId: ReturnType<typeof setTimeout>;
};

type ActiveConnection = {
  transport: WebTransportLike;
  streamReader: ReadableStreamDefaultReader<ReadableStream<Uint8Array>> | null;
  intentional: boolean;
  closeNotified: boolean;
};

const COMMUNICATION_TYPES = [
  "error",
  "error_protocol",
  "error_anonymous",
  "error_internal",
  "error_invalid_data",
  "error_invalid_user_id",
  "error_invalid_omikron_id",
  "error_not_found",
  "error_not_authenticated",
  "error_no_iota",
  "error_invalid_challenge",
  "error_invalid_secret",
  "error_invalid_private_key",
  "error_invalid_public_key",
  "error_no_user_id",
  "error_no_call_id",
  "error_invalid_call_id",
  "success",
  "shorten_link",
  "settings_save",
  "settings_load",
  "settings_list",
  "message",
  "message_state",
  "message_send",
  "message_live",
  "message_other_iota",
  "message_chunk",
  "messages_get",
  "push_notification",
  "read_notification",
  "get_notifications",
  "change_confirm",
  "confirm_receive",
  "confirm_read",
  "get_chats",
  "get_states",
  "add_community",
  "remove_community",
  "get_communities",
  "challenge",
  "challenge_response",
  "register",
  "register_response",
  "identification",
  "identification_response",
  "register_iota",
  "register_iota_success",
  "ping",
  "pong",
  "add_conversation",
  "send_chat",
  "client_changed",
  "client_connected",
  "client_disconnected",
  "client_closed",
  "public_key",
  "private_key",
  "webrtc_sdp",
  "webrtc_ice",
  "start_stream",
  "end_stream",
  "watch_stream",
  "call_token",
  "call_invite",
  "call_disconnect_user",
  "call_timeout_user",
  "call_set_anonymous_joining",
  "end_call",
  "function",
  "update",
  "create_user",
  "rho_update",
  "user_connected",
  "user_disconnected",
  "iota_connected",
  "iota_disconnected",
  "sync_client_iota_status",
  "get_user_data",
  "get_iota_data",
  "iota_user_data",
  "change_user_data",
  "change_iota_data",
  "get_register",
  "complete_register_user",
  "complete_register_iota",
  "delete_user",
  "delete_iota",
  "start_register",
  "complete_register",
] as const;

const DATA_TYPES = [
  "error_type",
  "error_protocol",
  "accepted_ids",
  "uuid",
  "register_id",
  "link",
  "settings",
  "settings_name",
  "chat_partner_id",
  "chat_partner_name",
  "iota_id",
  "user_id",
  "user_ids",
  "iota_ids",
  "user_state",
  "user_states",
  "user_pings",
  "call_state",
  "screen_share",
  "private_key_hash",
  "accepted",
  "accepted_profiles",
  "denied_profiles",
  "content",
  "messages",
  "notifications",
  "send_time",
  "get_time",
  "get_variant",
  "shared_secret_own",
  "shared_secret_other",
  "shared_secret_sign",
  "shared_secret",
  "call_id",
  "call_token",
  "untill",
  "enabled",
  "start_date",
  "end_date",
  "receiver_id",
  "sender_id",
  "signature",
  "signed",
  "message",
  "message_state",
  "last_ping",
  "ping_iota",
  "ping_clients",
  "matches",
  "omikron",
  "offset",
  "amount",
  "position",
  "name",
  "path",
  "codec",
  "function",
  "payload",
  "result",
  "interactables",
  "want_to_watch",
  "watcher",
  "created_at",
  "username",
  "display",
  "avatar",
  "about",
  "status",
  "public_key",
  "sub_level",
  "sub_end",
  "community_address",
  "challenge",
  "community_title",
  "communities",
  "rho_connections",
  "user",
  "online_status",
  "omikron_id",
  "omikron_connections",
  "reset_token",
  "new_token",
  "call_invited",
  "call_members",
  "calls",
  "timeout",
  "has_admin",
] as const;

const COMMUNICATION_TYPE_BY_NAME = createIndexMap(COMMUNICATION_TYPES);
const DATA_TYPE_BY_NAME = createIndexMap(DATA_TYPES);
const SCALAR_NUMBER_ARRAY_DATA_TYPES = new Set(["last_ping", "ping_iota"]);

const dataKindByType = new Map<string, DataKind>();

registerDataKinds("number", [
  "user_id",
  "sender_id",
  "register_id",
  "receiver_id",
  "call_id",
  "amount",
  "position",
  "offset",
  "timeout",
  "iota_id",
  "chat_partner_id",
  "untill",
  "start_date",
  "end_date",
  "omikron_id",
  "send_time",
  "sub_level",
]);

registerDataKinds("string", [
  "error_type",
  "username",
  "display",
  "avatar",
  "about",
  "public_key",
  "message",
  "content",
  "path",
  "codec",
  "function",
  "uuid",
  "link",
  "settings_name",
  "chat_partner_name",
  "user_state",
  "call_state",
  "private_key_hash",
  "name",
  "shared_secret_own",
  "shared_secret_other",
  "shared_secret_sign",
  "shared_secret",
  "message_state",
  "signature",
  "reset_token",
  "new_token",
  "call_token",
  "challenge",
]);

registerDataKinds({ array: "container" }, [
  "messages",
  "communities",
  "rho_connections",
  "matches",
]);

registerDataKinds({ array: "number" }, [
  "notifications",
  "iota_ids",
  "user_ids",
  "accepted_ids",
  "last_ping",
  "ping_iota",
  "get_time",
  "omikron_connections",
]);

registerDataKinds("container", [
  "settings",
  "user",
  "payload",
  "result",
  "ping_clients",
  "user_pings",
]);

registerDataKinds("bool", [
  "enabled",
  "signed",
  "accepted",
  "has_admin",
  "screen_share",
]);

registerDataKinds({ array: "string" }, ["user_states"]);

registerDataKinds("null", [
  "error_protocol",
  "accepted_profiles",
  "denied_profiles",
  "get_variant",
  "omikron",
  "interactables",
  "want_to_watch",
  "watcher",
  "created_at",
  "status",
  "sub_end",
  "community_address",
  "community_title",
  "online_status",
  "call_invited",
  "call_members",
  "calls",
]);

for (const dataType of DATA_TYPES) {
  if (!dataKindByType.has(dataType)) {
    throw new Error(`Missing data kind mapping for "${dataType}"`);
  }
}

export type TypedMessage<TData = Record<string, unknown>> = {
  id: number;
  type: string;
  data: TData;
};

export type Message = TypedMessage;

export type SchemaMap = Record<
  string,
  { request: z.ZodType; response: z.ZodType }
>;

type SendOptions = {
  id?: number;
  noResponse?: boolean;
};

export type BoundSendFn<T extends SchemaMap> = {
  <K extends keyof T & string>(
    type: K,
    data: z.input<T[K]["request"]>,
    options: { id?: number; noResponse: true },
  ): Promise<void>;

  <K extends keyof T & string>(
    type: K,
    data: z.input<T[K]["request"]>,
    options?: { id?: number; noResponse?: false },
  ): Promise<TypedMessage<z.output<T[K]["response"]>>>;
};

export type PushHandler<TData = Record<string, unknown>> = (
  message: TypedMessage<TData>,
) => void;

export type TransportCloseEvent = {
  error?: unknown;
  intentional: boolean;
};

type TransportClientOptions = {
  url?: string;
  onReadyStateChange?: (readyState: number) => void;
  onClose?: (event: TransportCloseEvent) => void;
};

export type TransportClient<T extends SchemaMap> = {
  connect(url?: string): Promise<void>;
  close(reason?: string): Promise<void>;
  send: BoundSendFn<T>;
  readyState(): number;
  subscribePush(handler: PushHandler): () => void;
};

export function createTransportClient<T extends SchemaMap>(
  schemas: T,
  options: TransportClientOptions = {},
): TransportClient<T> {
  const pending = new Map<number, PendingRequest>();
  const pushHandlers = new Set<PushHandler>();

  let currentConnection: ActiveConnection | null = null;
  let currentReadyState: number = READY_STATE.CLOSED;
  let nextRequestId = 1;
  let configuredUrl = options.url;

  const setReadyState = (readyState: number) => {
    currentReadyState = readyState;
    options.onReadyStateChange?.(readyState);
  };

  const rejectPending = (reason: unknown) => {
    for (const [id, request] of pending) {
      clearTimeout(request.timeoutId);
      request.reject(reason);
      pending.delete(id);
    }
  };

  const notifyClosed = (connection: ActiveConnection, error?: unknown) => {
    if (connection.closeNotified) {
      return;
    }

    connection.closeNotified = true;

    if (currentConnection === connection) {
      currentConnection = null;
    }

    if (currentReadyState !== READY_STATE.CLOSED) {
      setReadyState(READY_STATE.CLOSED);
    }

    rejectPending(error ?? new Error("Transport closed"));
    options.onClose?.({ error, intentional: connection.intentional });
  };

  const handleConnectionFailure = (connection: ActiveConnection, error?: unknown) => {
    if (currentConnection !== connection && connection.closeNotified) {
      return;
    }

    notifyClosed(connection, error);
  };

  const handleIncomingMessage = (message: TypedMessage) => {
    if (message.type !== "pong") {
      log(2, "Socket", "blue", message.type, message.data);
    }

    if (message.id !== 0) {
      const pendingRequest = pending.get(message.id);

      if (pendingRequest) {
        clearTimeout(pendingRequest.timeoutId);
        pending.delete(message.id);

        if (message.type.startsWith("error")) {
          pendingRequest.reject(message);
          return;
        }

        const schema = schemas[pendingRequest.requestType];
        if (!schema) {
          pendingRequest.resolve(message);
          return;
        }

        const result = schema.response.safeParse(message.data);
        if (result.success) {
          pendingRequest.resolve({
            ...message,
            data: result.data as Record<string, unknown>,
          });
          return;
        }

        log(
          0,
          "Socket",
          "red",
          `Response validation failed for "${message.type}"`,
          result.error,
          message.data,
        );
        pendingRequest.reject(
          new Error(
            `Response validation failed for "${message.type}": ${result.error.message}`,
          ),
        );
        return;
      }
    }

    const schema = schemas[message.type];
    if (schema) {
      const result = schema.response.safeParse(message.data);
      if (!result.success) {
        log(
          0,
          "Socket",
          "red",
          `Push-event validation failed for "${message.type}"`,
          result.error,
          message.data,
        );
        return;
      }

      message = {
        ...message,
        data: result.data as Record<string, unknown>,
      };
    }

    for (const handler of pushHandlers) {
      handler(message);
    }
  };

  const startIncomingLoop = (connection: ActiveConnection) => {
    connection.streamReader =
      connection.transport.incomingUnidirectionalStreams.getReader();

    void (async () => {
      try {
        while (currentConnection === connection && !connection.intentional) {
          const result = await connection.streamReader?.read();
          if (!result || result.done) {
            break;
          }

          const frame = await readFrame(result.value);
          if (frame === null) {
            try {
              connection.transport.close({
                closeCode: APPLICATION_CLOSE_CODE,
                reason: APPLICATION_CLOSE_REASON,
              });
            } catch {
              // Ignore close errors during peer shutdown.
            }

            handleConnectionFailure(connection, new Error("Transport closed by peer"));
            return;
          }

          handleIncomingMessage(frame);
        }

        if (!connection.intentional) {
          handleConnectionFailure(connection, new Error("Transport stream closed"));
        }
      } catch (error) {
        log(0, "Socket", "red", "Incoming transport stream failed", error);
        handleConnectionFailure(connection, error);
      } finally {
        connection.streamReader?.releaseLock();
        connection.streamReader = null;
      }
    })();
  };

  const awaitClosed = (connection: ActiveConnection) => {
    void connection.transport.closed
      .then(() => {
        handleConnectionFailure(connection);
      })
      .catch((error) => {
        handleConnectionFailure(connection, error);
      });
  };

  const connect = async (url = configuredUrl) => {
    if (!url) {
      throw new Error("Transport URL is not configured");
    }

    configuredUrl = url;

    if (currentConnection) {
      await close("reconnect");
    }

    const WebTransportCtor = getWebTransportCtor();
    const transport = new WebTransportCtor(url);
    const connection: ActiveConnection = {
      transport,
      streamReader: null,
      intentional: false,
      closeNotified: false,
    };

    currentConnection = connection;
    setReadyState(READY_STATE.CONNECTING);
    awaitClosed(connection);

    try {
      await transport.ready;

      if (currentConnection !== connection) {
        return;
      }

      log(1, "Socket", "green", "Connected");
      setReadyState(READY_STATE.OPEN);
      startIncomingLoop(connection);
    } catch (error) {
      log(0, "Socket", "red", "WebTransport connection failed", error);
      handleConnectionFailure(connection, error);
      throw error;
    }
  };

  const close = async (reason = APPLICATION_CLOSE_REASON) => {
    const connection = currentConnection;
    if (!connection) {
      setReadyState(READY_STATE.CLOSED);
      return;
    }

    connection.intentional = true;
    setReadyState(READY_STATE.CLOSING);

    rejectPending(new Error("Transport closed"));

    try {
      await writeCloseFrame(connection.transport);
    } catch (error) {
      log(1, "Socket", "yellow", "Failed to send close sentinel", error);
    }

    try {
      connection.streamReader?.cancel().catch(() => undefined);
    } catch {
      // Ignore reader cancellation failures during shutdown.
    }

    try {
      connection.transport.close({
        closeCode: APPLICATION_CLOSE_CODE,
        reason,
      });
    } catch {
      // Ignore close errors during shutdown.
    }

    try {
      await connection.transport.closed.catch(() => undefined);
    } finally {
      notifyClosed(connection);
    }
  };

  const send: BoundSendFn<T> = ((
    type: string,
    input?: Record<string, unknown>,
    options?: SendOptions,
  ): Promise<unknown> => {
    if (!currentConnection || currentReadyState !== READY_STATE.OPEN) {
      return Promise.reject(new Error("Transport is not connected"));
    }

    const connection = currentConnection;

    try {
      const schema = schemas[type];
      let payload: Record<string, unknown>;

      if (schema) {
        const result = schema.request.safeParse(input ?? {});
        if (!result.success) {
          log(
            0,
            "Socket",
            "red",
            `Request validation failed for "${type}"`,
            result.error,
          );
          return Promise.reject(
            new Error(
              `Request validation failed for "${type}": ${result.error.message}`,
            ),
          );
        }

        payload = coercePayload(result.data);
      } else {
        payload = coercePayload(input ?? {});
      }

      if (type !== "ping") {
        log(2, "Socket", "purple", type, payload);
      }

      const expectsResponse = !options?.noResponse;
      const requestId = resolveRequestId(options?.id, expectsResponse, pending, () => {
        const current = nextRequestId;
        nextRequestId = current >= MAX_REQUEST_ID ? 1 : current + 1;
        return current;
      });

      const messageBytes = encodeCommunicationMessage({
        id: requestId,
        type,
        data: payload,
      });

      if (!expectsResponse) {
        return writeMessage(connection.transport, messageBytes);
      }

      return new Promise<TypedMessage>((resolve, reject) => {
        const timeoutId = setTimeout(() => {
          pending.delete(requestId);
          reject(
            new Error(`Request "${type}" timed out after ${RESPONSE_TIMEOUT}ms`),
          );
        }, RESPONSE_TIMEOUT);

        pending.set(requestId, {
          requestType: type,
          resolve,
          reject,
          timeoutId,
        });

        void writeMessage(connection.transport, messageBytes).catch((error) => {
          clearTimeout(timeoutId);
          pending.delete(requestId);
          reject(error);
        });
      });
    } catch (error) {
      return Promise.reject(error);
    }
  }) as BoundSendFn<T>;

  return {
    connect,
    close,
    send,
    readyState: () => currentReadyState,
    subscribePush(handler: PushHandler) {
      pushHandlers.add(handler);
      return () => {
        pushHandlers.delete(handler);
      };
    },
  };
}

function createIndexMap(values: readonly string[]) {
  const map = new Map<string, number>();

  values.forEach((value, index) => {
    map.set(normalizeName(value), index);
  });

  return map;
}

function registerDataKinds(kind: DataKind, names: readonly string[]) {
  for (const name of names) {
    if (dataKindByType.has(name)) {
      throw new Error(`Duplicate data kind registration for "${name}"`);
    }

    dataKindByType.set(name, kind);
  }
}

function getWebTransportCtor() {
  const ctor = (globalThis as WebTransportGlobal).WebTransport;
  if (!ctor) {
    throw new Error("WebTransport is not available in this environment");
  }

  return ctor;
}

function normalizeName(value: string) {
  return value.toLowerCase().replaceAll("_", "");
}

function coercePayload(value: unknown): Record<string, unknown> {
  if (!isPlainObject(value)) {
    throw new Error("Protocol payload must be a plain object");
  }

  return value;
}

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function resolveRequestId(
  requestedId: number | undefined,
  expectsResponse: boolean,
  pending: Map<number, PendingRequest>,
  nextId: () => number,
) {
  if (requestedId !== undefined) {
    validateRequestId(requestedId, expectsResponse);
    if (expectsResponse && pending.has(requestedId)) {
      throw new Error(`Request id ${requestedId} is already pending`);
    }

    return requestedId;
  }

  if (!expectsResponse) {
    return 0;
  }

  let attempts = 0;
  let candidate = nextId();

  while (candidate === 0 || pending.has(candidate)) {
    candidate = nextId();
    attempts += 1;

    if (attempts > MAX_REQUEST_ID) {
      throw new Error("Unable to allocate a free request id");
    }
  }

  return candidate;
}

function validateRequestId(id: number, expectsResponse: boolean) {
  if (!Number.isInteger(id) || id < 0 || id > MAX_REQUEST_ID) {
    throw new Error(`Request id must be a u32 between 0 and ${MAX_REQUEST_ID}`);
  }

  if (expectsResponse && id === 0) {
    throw new Error("Request id 0 cannot be used when a response is expected");
  }
}

async function writeMessage(transport: WebTransportLike, payload: Uint8Array) {
  if (payload.byteLength >= CLOSE_FRAME_LEN) {
    throw new Error("Message too large for transport frame");
  }

  const stream = await transport.createUnidirectionalStream();
  const writer = stream.getWriter();

  try {
    const frame = new Uint8Array(4 + payload.byteLength);
    writeU32(frame, 0, payload.byteLength);
    frame.set(payload, 4);

    await writer.write(frame);
    await writer.close();
  } finally {
    writer.releaseLock();
  }
}

async function writeCloseFrame(transport: WebTransportLike) {
  const stream = await transport.createUnidirectionalStream();
  const writer = stream.getWriter();

  try {
    const frame = new Uint8Array(4);
    writeU32(frame, 0, CLOSE_FRAME_LEN);
    await writer.write(frame);
    await writer.close();
  } finally {
    writer.releaseLock();
  }
}

async function readFrame(stream: ReadableStream<Uint8Array>) {
  const payload = await readAll(stream);
  if (payload.byteLength < 4) {
    throw new Error("Received truncated transport frame");
  }

  const declaredLength = readU32(payload, 0);
  if (declaredLength === CLOSE_FRAME_LEN) {
    return null;
  }

  const actualLength = payload.byteLength - 4;
  if (actualLength !== declaredLength) {
    throw new Error(
      `Transport frame length mismatch: expected ${declaredLength}, received ${actualLength}`,
    );
  }

  return decodeCommunicationMessage(payload.subarray(4));
}

async function readAll(stream: ReadableStream<Uint8Array>) {
  const reader = stream.getReader();
  const chunks: Uint8Array[] = [];
  let totalLength = 0;

  try {
    while (true) {
      const { value, done } = await reader.read();
      if (done) {
        break;
      }

      chunks.push(value);
      totalLength += value.byteLength;
    }
  } finally {
    reader.releaseLock();
  }

  const buffer = new Uint8Array(totalLength);
  let offset = 0;

  for (const chunk of chunks) {
    buffer.set(chunk, offset);
    offset += chunk.byteLength;
  }

  return buffer;
}

function encodeCommunicationMessage(message: TypedMessage<Record<string, unknown>>) {
  const typeIndex = parseCommunicationType(message.type);
  const dataBuffer = encodeValueForKind("container", message.data, "payload");
  const hasId = message.id !== 0;
  const totalLength = 2 + (hasId ? 4 : 0) + 4 + dataBuffer.byteLength;
  const buffer = new Uint8Array(totalLength);

  buffer[0] = typeIndex;
  buffer[1] = hasId ? 0b0010_0000 : 0;

  let offset = 2;
  if (hasId) {
    writeU32(buffer, offset, message.id);
    offset += 4;
  }

  writeU32(buffer, offset, dataBuffer.byteLength);
  offset += 4;
  buffer.set(dataBuffer, offset);

  return buffer;
}

function decodeCommunicationMessage(payload: Uint8Array): TypedMessage {
  const reader = new ByteReader(payload);
  const typeIndex = reader.readU8();
  const flags = reader.readU8();
  const hasSender = (flags & 0b1000_0000) !== 0;
  const hasReceiver = (flags & 0b0100_0000) !== 0;
  const hasId = (flags & 0b0010_0000) !== 0;

  const id = hasId ? reader.readU32() : 0;
  if (hasSender) {
    reader.readU48();
  }
  if (hasReceiver) {
    reader.readU48();
  }

  const dataLength = reader.readU32();
  const dataBytes = reader.readBytes(dataLength);
  const decodedData = decodeValue(new ByteReader(dataBytes));
  if (!isPlainObject(decodedData)) {
    throw new Error("Protocol payload container was not decoded as an object");
  }

  if (!reader.isAtEnd()) {
    throw new Error("Trailing bytes found after communication payload");
  }

  return {
    id,
    type: COMMUNICATION_TYPES[typeIndex] ?? "error_protocol",
    data: decodedData,
  };
}

function parseCommunicationType(type: string) {
  const index = COMMUNICATION_TYPE_BY_NAME.get(normalizeName(type));
  if (index === undefined) {
    throw new Error(`Unknown communication type "${type}"`);
  }

  return index;
}

function parseDataType(type: string) {
  const index = DATA_TYPE_BY_NAME.get(normalizeName(type));
  if (index === undefined) {
    throw new Error(`Unknown data type "${type}"`);
  }

  return {
    index,
    name: DATA_TYPES[index],
  };
}

function getExpectedKind(type: string) {
  const kind = dataKindByType.get(type);
  if (!kind) {
    throw new Error(`No data kind registered for "${type}"`);
  }

  return kind;
}

function encodeValueForKind(kind: DataKind, value: unknown, path: string): Uint8Array {
  if (typeof kind === "object") {
    return encodeArrayValue(kind.array, value, path);
  }

  switch (kind) {
    case "bool":
      if (typeof value !== "boolean") {
        throw new Error(`Expected boolean at "${path}"`);
      }

      return Uint8Array.of(0x03, value ? 1 : 0);

    case "number":
      return encodeNumberValue(value, path);

    case "string":
      if (typeof value !== "string") {
        throw new Error(`Expected string at "${path}"`);
      }

      return encodeStringValue(value);

    case "container":
      if (!isPlainObject(value)) {
        throw new Error(`Expected object at "${path}"`);
      }

      return encodeContainerValue(value, path);

    case "null":
      if (value !== null && value !== undefined) {
        throw new Error(`Expected null at "${path}"`);
      }

      return Uint8Array.of(0x06);
  }
}

function encodeNumberValue(value: unknown, path: string) {
  if (typeof value !== "number" || !Number.isFinite(value) || !Number.isSafeInteger(value)) {
    throw new Error(`Expected safe integer at "${path}"`);
  }

  const buffer = new Uint8Array(9);
  buffer[0] = 0x01;
  writeI64(buffer, 1, value);
  return buffer;
}

function encodeStringValue(value: string) {
  const bytes = textEncoder.encode(value);
  const buffer = new Uint8Array(5 + bytes.byteLength);
  buffer[0] = 0x02;
  writeU32(buffer, 1, bytes.byteLength);
  buffer.set(bytes, 5);
  return buffer;
}

function encodeArrayValue(
  innerKind: PrimitiveDataKind | "container" | "null",
  value: unknown,
  path: string,
) {
  if (!Array.isArray(value)) {
    throw new Error(`Expected array at "${path}"`);
  }

  if (value.length > 0xffff) {
    throw new Error(`Array at "${path}" is too large for protocol encoding`);
  }

  const encodedItems = value.map((entry, index) =>
    encodeValueForKind(innerKind, entry, `${path}[${index}]`),
  );

  const byteLength = encodedItems.reduce((sum, item) => sum + item.byteLength, 0);
  const buffer = new Uint8Array(4 + byteLength);
  buffer[0] = 0x04;
  buffer[1] = value.length === 0 ? 0x00 : kindToMarker(innerKind);
  writeU16(buffer, 2, value.length);

  let offset = 4;
  for (const item of encodedItems) {
    buffer.set(item, offset);
    offset += item.byteLength;
  }

  return buffer;
}

function encodeContainerValue(value: Record<string, unknown>, path: string) {
  const normalizedEntries = new Map<string, { index: number; value: unknown }>();

  for (const [rawKey, rawValue] of Object.entries(value)) {
    const parsedType = parseDataType(rawKey);
    normalizedEntries.set(parsedType.name, {
      index: parsedType.index,
      value: normalizeOutgoingValue(parsedType.name, rawValue),
    });
  }

  const entries = [...normalizedEntries.entries()].sort(
    ([, left], [, right]) => left.index - right.index,
  );

  const encodedEntries: Uint8Array[] = [];
  let totalLength = 3;

  for (const [name, entry] of entries) {
    const expectedKind = getExpectedKind(name);
    const pathForEntry = `${path}.${name}`;

    if (expectedKind === "bool") {
      if (typeof entry.value !== "boolean") {
        throw new Error(`Expected boolean at "${pathForEntry}"`);
      }

      const encoded = Uint8Array.of(entry.index, entry.value ? 1 : 0);
      encodedEntries.push(encoded);
      totalLength += encoded.byteLength;
      continue;
    }

    const encodedValue = encodeValueForKind(expectedKind, entry.value, pathForEntry);
    const body = encodedValue.subarray(1);
    const encoded = new Uint8Array(5 + body.byteLength);
    encoded[0] = entry.index;
    writeU32(encoded, 1, body.byteLength);
    encoded.set(body, 5);
    encodedEntries.push(encoded);
    totalLength += encoded.byteLength;
  }

  const buffer = new Uint8Array(totalLength);
  buffer[0] = 0x05;
  writeU16(buffer, 1, entries.length);

  let offset = 3;
  for (const encoded of encodedEntries) {
    buffer.set(encoded, offset);
    offset += encoded.byteLength;
  }

  return buffer;
}

function decodeValue(reader: ByteReader): unknown {
  const marker = reader.readU8();

  switch (marker) {
    case 0x01:
      return reader.readI64();

    case 0x02: {
      const length = reader.readU32();
      return textDecoder.decode(reader.readBytes(length));
    }

    case 0x03:
      return reader.readU8() !== 0;

    case 0x04: {
      reader.readU8();
      const length = reader.readU16();
      const values: unknown[] = [];

      for (let index = 0; index < length; index += 1) {
        values.push(decodeValue(reader));
      }

      return values;
    }

    case 0x05:
      return decodeContainer(reader);

    case 0x06:
      return null;

    default:
      throw new Error(`Unknown data value marker 0x${marker.toString(16)}`);
  }
}

function decodeContainer(reader: ByteReader) {
  const length = reader.readU16();
  const value: Record<string, unknown> = {};

  for (let index = 0; index < length; index += 1) {
    const keyIndex = reader.readU8();
    const key = DATA_TYPES[keyIndex];
    if (!key) {
      throw new Error(`Unknown data type index ${keyIndex}`);
    }

    const expectedKind = getExpectedKind(key);

    if (expectedKind === "bool") {
      value[key] = normalizeIncomingValue(key, reader.readU8() !== 0);
      continue;
    }

    const encodedLength = reader.readU32();
    const encodedValue = reader.readBytes(encodedLength);
    const fullValue = new Uint8Array(1 + encodedValue.byteLength);
    fullValue[0] = kindToMarker(expectedKind);
    fullValue.set(encodedValue, 1);

    value[key] = normalizeIncomingValue(
      key,
      decodeValue(new ByteReader(fullValue)),
    );
  }

  return value;
}

function normalizeOutgoingValue(type: string, value: unknown) {
  if (SCALAR_NUMBER_ARRAY_DATA_TYPES.has(type) && typeof value === "number") {
    return [value];
  }

  return value;
}

function normalizeIncomingValue(type: string, value: unknown) {
  if (
    SCALAR_NUMBER_ARRAY_DATA_TYPES.has(type) &&
    Array.isArray(value) &&
    value.length === 1 &&
    typeof value[0] === "number"
  ) {
    return value[0];
  }

  return value;
}

function kindToMarker(kind: DataKind) {
  if (typeof kind === "object") {
    return 0x04;
  }

  switch (kind) {
    case "number":
      return 0x01;
    case "string":
      return 0x02;
    case "bool":
      return 0x03;
    case "container":
      return 0x05;
    case "null":
      return 0x06;
  }
}

function writeU16(buffer: Uint8Array, offset: number, value: number) {
  new DataView(buffer.buffer, buffer.byteOffset, buffer.byteLength).setUint16(
    offset,
    value,
    false,
  );
}

function writeU32(buffer: Uint8Array, offset: number, value: number) {
  new DataView(buffer.buffer, buffer.byteOffset, buffer.byteLength).setUint32(
    offset,
    value,
    false,
  );
}

function writeI64(buffer: Uint8Array, offset: number, value: number) {
  new DataView(buffer.buffer, buffer.byteOffset, buffer.byteLength).setBigInt64(
    offset,
    BigInt(value),
    false,
  );
}

function readU32(buffer: Uint8Array, offset: number) {
  return new DataView(buffer.buffer, buffer.byteOffset, buffer.byteLength).getUint32(
    offset,
    false,
  );
}

class ByteReader {
  private readonly view: DataView;

  private offset = 0;

  constructor(private readonly bytes: Uint8Array) {
    this.view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  }

  readU8() {
    this.ensureAvailable(1);
    const value = this.view.getUint8(this.offset);
    this.offset += 1;
    return value;
  }

  readU16() {
    this.ensureAvailable(2);
    const value = this.view.getUint16(this.offset, false);
    this.offset += 2;
    return value;
  }

  readU32() {
    this.ensureAvailable(4);
    const value = this.view.getUint32(this.offset, false);
    this.offset += 4;
    return value;
  }

  readI64() {
    this.ensureAvailable(8);
    const value = this.view.getBigInt64(this.offset, false);
    this.offset += 8;

    const numberValue = Number(value);
    if (!Number.isSafeInteger(numberValue)) {
      throw new Error(`Decoded number ${value.toString()} exceeds JS safe integer range`);
    }

    return numberValue;
  }

  readU48() {
    this.ensureAvailable(6);
    const upper = this.view.getUint16(this.offset, false);
    const lower = this.view.getUint32(this.offset + 2, false);
    this.offset += 6;
    return upper * 2 ** 32 + lower;
  }

  readBytes(length: number) {
    this.ensureAvailable(length);
    const value = this.bytes.subarray(this.offset, this.offset + length);
    this.offset += length;
    return value;
  }

  isAtEnd() {
    return this.offset === this.bytes.byteLength;
  }

  private ensureAvailable(length: number) {
    if (this.offset + length > this.bytes.byteLength) {
      throw new Error("Unexpected end of protocol buffer");
    }
  }
}