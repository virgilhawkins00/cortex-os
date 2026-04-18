import React, { useEffect, useState, useRef } from 'react';
import { useNatsStore } from './store/useNatsStore';
import { LayoutDashboard, Brain, Database, Settings, Terminal, Send, Activity, ShieldCheck } from 'lucide-react';

function App() {
  const { messages, sendMessage, connect, connected, status } = useNatsStore();
  const [inputText, setInputText] = useState('');
  const [activeTab, setActiveTab] = useState('agents');
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    // Connect to NATS WebSocket port
    const natsUrl = `ws://${window.location.hostname}:4223`;
    connect(natsUrl);
  }, [connect]);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages]);

  const handleSend = () => {
    if (inputText.trim()) {
      sendMessage(inputText);
      setInputText('');
    }
  };

  return (
    <div className="app-container">
      {/* ─── Sidebar ───────────────────────────────── */}
      <aside className="sidebar">
        <div className="logo">
          <Brain size={28} />
          <span>Cortex OS</span>
        </div>
        
        <nav className="nav-links">
          <div className={`nav-item ${activeTab === 'agents' ? 'active' : ''}`} onClick={() => setActiveTab('agents')}>
            <Activity size={20} />
            Agents
          </div>
          <div className={`nav-item ${activeTab === 'memory' ? 'active' : ''}`} onClick={() => setActiveTab('memory')}>
            <Database size={20} />
            Memory
          </div>
          <div className={`nav-item ${activeTab === 'tools' ? 'active' : ''}`} onClick={() => setActiveTab('tools')}>
            <Terminal size={20} />
            Tools
          </div>
          <div className={`nav-item ${activeTab === 'config' ? 'active' : ''}`} onClick={() => setActiveTab('config')}>
            <Settings size={20} />
            Config
          </div>
        </nav>

        <div style={{ marginTop: 'auto', padding: '1rem', background: 'rgba(255,255,255,0.02)', borderRadius: '8px' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', marginBottom: '0.5rem' }}>
             <ShieldCheck size={16} color="var(--success-color)" />
             <span style={{ fontSize: '0.8rem', fontWeight: 600 }}>Policy: Full</span>
          </div>
          <div style={{ fontSize: '0.7rem', color: 'var(--text-secondary)' }}>
            Workspace: cortex-os/v0.1.0
          </div>
        </div>
      </aside>

      {/* ─── Main Content ─────────────────────────── */}
      <main className="main-content">
        <header className="top-bar glass">
          <h1 style={{ fontSize: '1.2rem', fontWeight: 700 }}>{activeTab.toUpperCase()}</h1>
          
          <div className="status-pills">
            <div className={`pill ${connected ? 'online' : 'offline'}`}>
              NATS: {connected ? 'CONNECTED' : 'OFFLINE'}
            </div>
            <div className={`pill ${status.brain ? 'online' : 'offline'}`}>
              BRAIN: {status.brain ? 'ONLINE' : 'OFFLINE'}
            </div>
          </div>
        </header>

        <div className="dashboard-grid">
          {/* ─── Chat Panel ─────────── */}
          <section className="chat-panel glass">
            <div className="messages" ref={scrollRef}>
              {messages.map((m) => (
                <div key={m.id} className={`message ${m.role}`}>
                  <div style={{ marginBottom: '0.5rem', fontWeight: 700, fontSize: '0.75rem', opacity: 0.5 }}>
                    {m.role.toUpperCase()} • {new Date(m.timestamp).toLocaleTimeString()}
                  </div>
                  {m.text}
                </div>
              ))}
            </div>
            
            <div className="input-area">
              <input 
                type="text" 
                placeholder="Type a goal or command..." 
                value={inputText}
                onChange={(e) => setInputText(e.target.value)}
                onKeyPress={(e) => e.key === 'Enter' && handleSend()}
              />
              <button className="btn-send" onClick={handleSend}>
                <Send size={20} />
              </button>
            </div>
          </section>

          {/* ─── Sidebar Info Panel ─────────── */}
          <section className="info-panel glass" style={{ padding: '1rem' }}>
            <h3 style={{ marginBottom: '1rem', borderBottom: '1px solid var(--border-color)', paddingBottom: '0.5rem' }}>
              System Insight
            </h3>
            
            <div style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
              <div className="glass" style={{ padding: '1rem', background: 'rgba(0,0,0,0.2)' }}>
                <div style={{ fontSize: '0.8rem', color: 'var(--text-secondary)', marginBottom: '0.5rem' }}>ACTIVE AGENT</div>
                <div style={{ fontWeight: 600 }}>Cortex-Core-1</div>
                <div style={{ fontSize: '0.75rem', marginTop: '0.5rem', color: 'var(--accent-cyan)' }}>● Running</div>
              </div>

              <div className="glass" style={{ padding: '1rem', background: 'rgba(0,0,0,0.2)' }}>
                <div style={{ fontSize: '0.8rem', color: 'var(--text-secondary)', marginBottom: '0.5rem' }}>MEMORY STATS</div>
                <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                  <span>Triples</span>
                  <span style={{ fontWeight: 600 }}>142</span>
                </div>
                <div style={{ display: 'flex', justifyContent: 'space-between', marginTop: '0.25rem' }}>
                  <span>Points</span>
                  <span style={{ fontWeight: 600 }}>2,840</span>
                </div>
              </div>
            </div>
          </section>
        </div>
      </main>
    </div>
  );
}

export default App;
