import { create } from 'zustand';
import { connect, JSONCodec, type NatsConnection } from 'nats.ws';

interface Message {
  id: string;
  role: 'agent' | 'user';
  text: string;
  timestamp: number;
}

interface NatsState {
  nc: NatsConnection | null;
  connected: bool;
  memories: any[];
  tools: string[];
  status: {
    nats: boolean;
    brain: boolean;
    memory: boolean;
    stats: {
      memories: number;
      triples: number;
    };
  };
  connect: (url: string, token?: string) => Promise<void>;
  sendMessage: (text: string) => Promise<void>;
  fetchMemories: () => Promise<void>;
  addMessage: (msg: Message) => void;
}

const jc = JSONCodec();

export const useNatsStore = create<NatsState>((set, get) => ({
  nc: null,
  connected: false,
  messages: [
    { id: '1', role: 'agent', text: 'Cortex OS initialized. How can I help you today?', timestamp: Date.now() }
  ],
  memories: [],
  tools: [],
  status: { 
    nats: false, 
    brain: false, 
    memory: false,
    stats: { memories: 0, triples: 0 }
  },

  connect: async (url: string, token?: string) => {
    try {
      const nc = await connect({
        servers: [url],
        token: token,
      });

      console.log('NATS Connected');
      set({ nc, connected: true, status: { ...get().status, nats: true } });

      // Subscribe to results
      const sub = nc.subscribe('cortex.result.web');
      (async () => {
        for await (const m of sub) {
          const data = jc.decode(m.data) as any;
          console.log('Received result:', data);
          get().addMessage({
            id: Math.random().toString(36),
            role: 'agent',
            text: data.output || data.response || JSON.stringify(data),
            timestamp: Date.now()
          });
        }
      })();

      // Interval check for brain/memory
      const checkHealth = async () => {
        try {
          const reply = await nc.request('cortex.brain.health', jc.encode({}), { timeout: 2000 });
          const data = jc.decode(reply.data) as any;
          const output = JSON.parse(data.output);
          set({ status: { 
            ...get().status, 
            brain: data.status === 'success',
            stats: {
              memories: output.memories || 0,
              triples: output.triples || 0
            }
          }});
        } catch (e) {
          set({ status: { ...get().status, brain: false } });
        }
      };

      setInterval(checkHealth, 5000);
      checkHealth();
      get().fetchMemories();

    } catch (err) {
      console.error('NATS Connection Error:', err);
      set({ connected: false });
    }
  },

  sendMessage: async (text: string) => {
    const { nc } = get();
    if (!nc) return;

    const userMsg: Message = {
      id: Math.random().toString(36),
      role: 'user',
      text,
      timestamp: Date.now()
    };
    get().addMessage(userMsg);

    // Send to brain
    await nc.publish('cortex.brain.think', jc.encode({
      prompt: text,
      include_memory: true,
      reply_subject: 'cortex.result.web'
    }));
  },

  fetchMemories: async () => {
    const { nc } = get();
    if (!nc) return;
    try {
      const reply = await nc.request('cortex.memory.list', jc.encode({ limit: 10 }), { timeout: 2000 });
      const data = jc.decode(reply.data) as any;
      const output = JSON.parse(data.output);
      set({ memories: output.memories || [] });
    } catch (e) {
      console.error('Failed to fetch memories:', e);
    }
  },

  addMessage: (msg: Message) => set((state) => ({ 
    messages: [...state.messages, msg] 
  })),
}));
