import React, { useEffect, useMemo, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/tauri'
import ItemCard from './components/ItemCard'
import { listen, UnlistenFn } from '@tauri-apps/api/event'
import { appWindow } from '@tauri-apps/api/window'

type ItemDto = {
  id: number
  created_at: number
  kind: 'text' | 'image' | 'file'
  size: number
  sha256_hex: string
  file_path?: string | null
  is_pinned: boolean
  preview?: string | null
}

type KindFilter = 'all' | 'text' | 'image' | 'file'

export default function App() {
  const [items, setItems] = useState<ItemDto[]>([])
  const [query, setQuery] = useState('')
  const [filter, setFilter] = useState<KindFilter>('all')
  const [selected, setSelected] = useState<number | null>(null)
  const [showSettings, setShowSettings] = useState(false)
  const [hotkey, setHotkey] = useState('')

  const filtered = useMemo(() => {
    let list = items
    if (filter !== 'all') list = list.filter(i => i.kind === filter)
    return list
  }, [items, filter])

  async function refreshRecent() {
    const list = await invoke<ItemDto[]>('list_recent', { limit: 100 })
    setItems(list)
  }

  async function doSearch() {
    const k = filter === 'all' ? null : filter
    const list = await invoke<ItemDto[]>('search', { query, kind: k, limit: 100 })
    setItems(list)
  }

  async function unlock() {
    try {
      await invoke('unlock')
    } catch (e) {
      console.error(e)
    }
  }

  async function copyItem(id: number) {
    await invoke('copy_item', { id })
    window.close()
  }

  async function pinItem(id: number, pin: boolean) {
    await invoke('pin_item', { id, pin })
    await refreshRecent()
  }

  async function deleteItem(id: number) {
    await invoke('delete_item', { id })
    await refreshRecent()
  }

  useEffect(() => {
    unlock().then(refreshRecent)
    ;(async () => {
      try {
        const s = await invoke<{ auto_lock_minutes: number, hotkey: string }>('get_settings')
        setHotkey(s.hotkey)
      } catch {}
    })()
  }, [])

  useEffect(() => {
    let unlisten: UnlistenFn | undefined
    ;(async () => {
      unlisten = await listen('items_updated', () => {
        refreshRecent()
      })
    })()
    return () => {
      if (unlisten) unlisten()
    }
  }, [])

  // expose reveal helper
  useEffect(() => {
    ;(window as any).__REVEAL = (path: string) => invoke('reveal_in_finder', { path })
    return () => { delete (window as any).__REVEAL }
  }, [])

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'ArrowDown') {
        e.preventDefault()
        const idx = filtered.findIndex(i => i.id === selected)
        const next = filtered[Math.min(filtered.length - 1, idx + 1)]?.id ?? filtered[0]?.id ?? null
        setSelected(next)
      } else if (e.key === 'ArrowUp') {
        e.preventDefault()
        const idx = filtered.findIndex(i => i.id === selected)
        const prev = filtered[Math.max(0, idx - 1)]?.id ?? filtered[filtered.length - 1]?.id ?? null
        setSelected(prev)
      } else if (e.key === 'Enter') {
        if (selected != null) copyItem(selected)
      } else if (e.key === 'Escape') {
        e.preventDefault();
        appWindow.hide()
      } else if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'p') {
        if (selected != null) {
          const it = items.find(i => i.id === selected)
          if (it) pinItem(selected, !it.is_pinned)
        }
      } else if (e.key === 'Delete') {
        if (selected != null) deleteItem(selected)
      }
    }
    window.addEventListener('keydown', onKey, { capture: true })
    document.addEventListener('keydown', onKey, { capture: true } as any)
    return () => {
      window.removeEventListener('keydown', onKey, { capture: true } as any)
      document.removeEventListener('keydown', onKey, { capture: true } as any)
    }
  }, [filtered, selected, items])

  const placeholder = 'Search clipboard…'

  return (
    <div className="wrap">
      <div className="topbar">
        <div className="spacer" />
        <button onClick={() => setShowSettings(s => !s)}>⚙ Settings</button>
      </div>
      <div className="searchRow">
        <input
          autoFocus
          placeholder={placeholder}
          value={query}
          onChange={e => setQuery(e.target.value)}
          onKeyUp={e => doSearch()}
        />
        <div className="filters">
          {(['all', 'text', 'image', 'file'] as KindFilter[]).map(k => (
            <button
              key={k}
              className={k === filter ? 'active' : ''}
              onClick={() => setFilter(k)}
            >
              {k[0].toUpperCase() + k.slice(1)}
            </button>
          ))}
        </div>
      </div>
      <div className="list">
        {filtered.map(it => (
          <ItemCard
            key={it.id}
            item={it}
            selected={it.id === selected}
            onClick={() => setSelected(it.id)}
            onCopy={() => copyItem(it.id)}
            onPin={() => pinItem(it.id, !it.is_pinned)}
            onDelete={() => deleteItem(it.id)}
          />
        ))}
        {filtered.length === 0 && <div className="empty">No items</div>}
      </div>
      {showSettings && (
        <div style={{ position: 'absolute', inset: 0, background: 'rgba(0,0,0,0.35)', display: 'flex', alignItems: 'center', justifyContent: 'center' }} onClick={() => setShowSettings(false)}>
          <div style={{ background: 'rgba(30,30,30,0.8)', border: '1px solid rgba(255,255,255,0.12)', borderRadius: 12, padding: 16, minWidth: 360 }} onClick={e => e.stopPropagation()}>
            <h3 style={{ marginTop: 0 }}>Settings</h3>
            <label>Global Hotkey</label>
            <input value={hotkey} onChange={e => setHotkey(e.target.value)} placeholder="CmdOrCtrl+Shift+Space" />
            <div style={{ marginTop: 12, display: 'flex', gap: 8, justifyContent: 'flex-end' }}>
              <button onClick={() => setShowSettings(false)}>Cancel</button>
              <button onClick={async () => { try { await invoke('set_hotkey', { hotkey }); setShowSettings(false) } catch {} }}>Save</button>
            </div>
            <div style={{ marginTop: 16, borderTop: '1px solid rgba(255,255,255,0.1)', paddingTop: 12 }}>
              <div style={{ fontWeight: 600, marginBottom: 8 }}>Danger Zone</div>
              <button style={{ background: 'rgba(255,0,0,0.2)', borderColor: 'rgba(255,0,0,0.4)' }} onClick={async () => {
                if (confirm('Reset master key? Existing encrypted items will become unreadable. This cannot be undone.')) {
                  try { await invoke('reset_master_key'); alert('Master key reset.'); } catch (e) { alert('Failed to reset key'); }
                }
              }}>Reset Master Key</button>
            </div>
            <div style={{ opacity: 0.7, marginTop: 8, fontSize: 12 }}>Examples: CmdOrCtrl+Shift+Space, Cmd+Alt+V</div>
          </div>
        </div>
      )}
    </div>
  )
}
