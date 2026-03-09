import { log } from "@tensamin/shared/log";
import {
  createContext,
  createEffect,
  createSignal,
  onCleanup,
  Show,
  untrack,
  useContext,
  type ParentProps,
} from "solid-js";

import { createTransportClient, READY_STATE, type BoundSendFn } from "./core";
import {
  PING_INTERVAL,
  RETRY_COUNT,
  RETRY_INTERVAL,
  TRANSPORT_URL,
} from "./values";
import {
  socket as schemas,
  type Socket as Schemas,
} from "@tensamin/shared/data";
import { useStorage } from "@tensamin/core-storage/context";
import { useCrypto } from "@tensamin/core-crypto/context";
import Loading from "@tensamin/ui/screens/loading";
import ErrorScreen from "@tensamin/ui/screens/error";
import { useNavigate } from "@solidjs/router";

type OmikronData = {
  id: number;
  public_key: string;
  ip_address: string;
};

type ContextType = {
  send: BoundSendFn<Schemas>;
  readyState: () => number;
  ownPing: () => number;
  iotaPing: () => number;
};

const socketContext = createContext<ContextType>();

export default function Provider(props: ParentProps) {
  const [omikron, setOmikron] = createSignal<OmikronData | null>(null);
  const [readyState, setReadyState] = createSignal<number>(READY_STATE.CLOSED);
  const [identified, setIdentified] = createSignal<boolean>(false);

  const [ownPing, setOwnPing] = createSignal<number>(0);
  const [iotaPing, setIotaPing] = createSignal<number>(0);

  const [error, setError] = createSignal<string>("");
  const [errorDescription, setErrorDescription] = createSignal<string>("");

  const { load } = useStorage();
  const { get_shared_secret, decrypt } = useCrypto();

  const navigate = useNavigate();

  let client: ReturnType<typeof createTransportClient<Schemas>> | null = null;

  // Load Omikron
  createEffect(() => {
    if (omikron()) return;

    const controller = new AbortController();

    (async () => {
      try {
        const userId = await load("user_id");

        // Redirect to login
        if (userId === 0) {
          navigate("/login");
          return;
        }

        const res = await fetch(
          "https://omega.tensamin.net/api/get/omikron/" + String(userId),
          { signal: controller.signal },
        );
        const data = await res.json();
        setOmikron(data);
      } catch (e) {
        if (controller.signal.aborted) return;

        setError("Failed to load Omikron data");
        setErrorDescription(
          "An error occurred while fetching the Omikron server data. Please try again later.",
        );
        log(0, "Socket", "red", "Failed to fetch Omikron data", e);
      }
    })();

    onCleanup(() => controller.abort());
  });

  createEffect(() => {
    if (identified()) {
      const interval = setInterval(async () => {
        try {
          const originalNow = Date.now();

          const data = await send("ping", {
            last_ping: originalNow,
          });

          const travelTime = Date.now() - originalNow;

          setOwnPing(travelTime);
          setIotaPing(data.data.ping_iota);
        } catch (error) {
          log(1, "Socket", "yellow", "Ping failed", error);
        }
      }, PING_INTERVAL);

      onCleanup(() => clearInterval(interval));
    }
  });

  // Create connection
  createEffect(() => {
    if (!omikron()) return;

    let attempts = 0;
    let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
    let disposed = false;

    const transportClient = createTransportClient(schemas, {
      url: TRANSPORT_URL,
      onReadyStateChange: setReadyState,
      onClose: ({ error: closeError, intentional }) => {
        setIdentified(false);

        if (disposed || intentional) {
          return;
        }

        log(0, "Socket", "red", "Disconnected", closeError);

        if (attempts < RETRY_COUNT) {
          attempts += 1;
          reconnectTimer = setTimeout(() => {
            void connect();
          }, RETRY_INTERVAL);
          return;
        }

        setError("Connection Failed");
        setErrorDescription(
          "Unable to connect to the server after multiple attempts. Please check your internet connection or try again later.",
        );
        log(0, "Socket", "red", "Reconnection attempts exhausted", closeError);
      },
    });

    client = transportClient;

    async function identify(activeClient: typeof transportClient) {
      const userId = await load("user_id");
      const privateKey = await load("private_key");
      const currentOmikron = untrack(() => omikron());

      activeClient
        .send("identification", { user_id: userId })
        .then(async (data) => {
          const ownUserData = await activeClient.send("get_user_data", {
            user_id: userId,
          });

          if (!currentOmikron?.public_key) {
            setError("Omikron data missing");
            setErrorDescription(
              "Omikron server data is missing. Please try again later.",
            );
            log(0, "Socket", "red", "Omikron public key missing");
            return;
          }

          try {
            const sharedSecret = await get_shared_secret(
              privateKey,
              ownUserData.data.public_key,
              currentOmikron.public_key,
            );

            const solvedChallenge = await decrypt(
              sharedSecret,
              data.data.challenge,
            );

            activeClient
              .send("challenge_response", {
                challenge: btoa(solvedChallenge),
              })
              .then(() => {
                if (client !== activeClient || disposed) {
                  return;
                }

                log(1, "Socket", "green", "Identification successful");
                setIdentified(true);
                setError("");
                setErrorDescription("");
              })
              .catch((error) => {
                log(0, "Socket", "red", "Challenge failed", error);
                setError("Challenge Failed");
                setErrorDescription(
                  "Failed to respond to the server's challenge. Please try again later.",
                );
              });
          } catch (err) {
            setError("Validation Failed");
            setErrorDescription(
              "Failed to validate the server's identity. Please try again later.",
            );
            log(0, "Socket", "red", "Server identity validation failed", err);
          }
        })
        .catch((e) => {
          log(0, "Socket", "red", "Identification failed", e);
          setError("Identification Failed");
          setErrorDescription(
            "Failed to identify with the server. Please try again later.",
          );
        });
    }

    async function connect() {
      if (disposed) return;

      try {
        await transportClient.connect(TRANSPORT_URL);
        attempts = 0;
        await identify(transportClient);
      } catch (error) {
        if (!disposed) {
          log(0, "Socket", "red", "Connection attempt failed", error);
        }
      }
    }

    void connect();

    onCleanup(() => {
      disposed = true;
      if (reconnectTimer) clearTimeout(reconnectTimer);
      if (client === transportClient) {
        client = null;
      }
      void transportClient.close("context-dispose");
      setReadyState(READY_STATE.CLOSED);
      setIdentified(false);
    });
  });

  // Create Send Function
  const send: BoundSendFn<Schemas> = ((
    type: string,
    data?: Record<string, unknown>,
    options?: { id?: number; noResponse?: boolean },
  ) => {
    if (!client) {
      return Promise.reject(new Error("Socket is not connected"));
    }

    if (options?.noResponse) {
      return client.send(type as keyof Schemas & string, data as never, {
        ...options,
        noResponse: true,
      });
    }
    return client.send(type as keyof Schemas & string, data as never, {
      ...options,
      noResponse: false,
    });
  }) as BoundSendFn<Schemas>;

  const progress = () => {
    if (!omikron()) return 40;
    if (readyState() !== READY_STATE.OPEN) return 70;
    if (!identified()) return 90;
    return 100;
  };

  return (
    <Show
      when={error() === "" && errorDescription() === ""}
      fallback={
        <ErrorScreen error={error()} description={errorDescription()} />
      }
    >
      <Show
        when={omikron() && identified()}
        fallback={<Loading progress={progress()} />}
      >
        <socketContext.Provider value={{ send, readyState, ownPing, iotaPing }}>
          {props.children}
        </socketContext.Provider>
      </Show>
    </Show>
  );
}

export function useSocket(): ContextType {
  const context = useContext(socketContext);
  if (!context)
    throw new Error("useSocket must be used within a SocketProvider");
  return context;
}
