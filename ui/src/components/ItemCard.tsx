import React from 'react'

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

export default function ItemCard({ item, selected, onClick, onCopy, onPin, onDelete }: {
  item: ItemDto
  selected?: boolean
  onClick?: () => void
  onCopy?: () => void
  onPin?: () => void
  onDelete?: () => void
}) {
  let title = ''
  if (item.kind === 'text') title = item.preview || 'Text'
  if (item.kind === 'image') title = 'Image'
  if (item.kind === 'file') title = (item.preview || item.file_path || 'File')

  const subtitle = item.kind === 'file'
    ? (item.file_path || '')
    : `Size: ${formatSize(item.size)}  •  Hash: ${item.sha256_hex.slice(0, 12)}`

  return (
    <div className={`card ${selected ? 'selected' : ''}`} onClick={onClick}>
      <div className="icon">
        {item.kind === 'text' && '🅣'}
        {item.kind === 'image' && '🖼️'}
        {item.kind === 'file' && '📄'}
      </div>
      <div className="body">
        <div className="row1">
          <div className="title">{title}</div>
          <div className="spacer" />
          {item.is_pinned && <div className="pin">📌</div>}
        </div>
        <div className="row2">{subtitle}</div>
        {item.kind === 'image' && <PreviewImage id={item.id} />}
        <div className="actions">
          <button onClick={e => { e.stopPropagation(); onCopy?.() }}>Copy</button>
          <button onClick={e => { e.stopPropagation(); onPin?.() }}>{item.is_pinned ? 'Unpin' : 'Pin'}</button>
          <button onClick={e => { e.stopPropagation(); onDelete?.() }}>Delete</button>
          {item.kind === 'file' && item.file_path && (
            <button onClick={e => {
              e.stopPropagation();
              window.__REVEAL?.(item.file_path!)
            }}>Reveal in Finder</button>
          )}
        </div>
      </div>
    </div>
  )
}

function formatSize(n: number): string {
  if (n <= 0) return '—'
  const units = ['B','KB','MB','GB','TB']
  let i = 0
  let x = n
  while (x >= 1024 && i < units.length - 1) { x /= 1024; i++ }
  return `${x.toFixed(i === 0 ? 0 : 1)} ${units[i]}`
}

function PreviewImage({ id }: { id: number }) {
  const [src, setSrc] = React.useState<string | null>(null)
  React.useEffect(() => {
    let alive = true
    import('@tauri-apps/api/tauri').then(({ invoke }) => {
      invoke<string>('get_image_preview', { id, max: 128 }).then((data) => {
        if (alive) setSrc(data)
      }).catch(() => {})
    })
    return () => { alive = false }
  }, [id])
  if (!src) return null
  return <img src={src} alt="preview" style={{ maxWidth: 128, maxHeight: 96, borderRadius: 6, marginTop: 6 }} />
}
