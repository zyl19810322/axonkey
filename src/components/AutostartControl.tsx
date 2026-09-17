import { useEffect, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { SettingsHelp } from './SettingsHelp'
import { disable, enable, isEnabled } from '@tauri-apps/plugin-autostart'

export function AutostartControl({ supported }: { supported: boolean }) {
  const [enabled, setEnabled] = useState<boolean | null>(null)
  const [silent, setSilent] = useState<boolean | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const [refreshVersion, setRefreshVersion] = useState(0)
  const inFlight = useRef(false)

  useEffect(() => {
    if (!supported) return
    let active = true
    let reading = false
    const refresh = async () => {
      if (inFlight.current || reading) return
      reading = true
      setBusy(true)
      try {
        const value = await isEnabled()
        const silentValue = await invoke<boolean>('get_silent_start')
        if (active) { setEnabled(value); setSilent(silentValue); setError('') }
      } catch (cause) {
        if (active) setError(`无法读取开机自启设置：${String(cause)}`)
      } finally {
        reading = false
        if (active) setBusy(false)
      }
    }
    void refresh()
    window.addEventListener('focus', refresh)
    return () => { active = false; window.removeEventListener('focus', refresh) }
  }, [supported, refreshVersion])

  const toggle = async () => {
    if (!supported || inFlight.current) return
    inFlight.current = true
    setBusy(true)
    setError('')
    try {
      // Read again before changing: the OS setting may have changed externally.
      const current = await isEnabled()
      if (current) await disable()
      else await enable()
      const actual = await isEnabled()
      setEnabled(actual)
      if (actual === current) setError('开机自启设置未生效，请重试。')
    } catch (cause) {
      setError(`无法更改开机自启设置：${String(cause)}`)
      try { setEnabled(await isEnabled()) } catch { setEnabled(null) }
    } finally {
      inFlight.current = false
      setBusy(false)
    }
  }

  const toggleSilent = async () => {
    if (!supported || inFlight.current || silent === null) return
    inFlight.current = true
    setBusy(true)
    setError('')
    try {
      const next = !silent
      await invoke('set_silent_start', { enabled: next })
      setSilent(await invoke<boolean>('get_silent_start'))
    } catch (cause) {
      setError(`无法更改静默启动设置：${String(cause)}`)
      try { setSilent(await invoke<boolean>('get_silent_start')) } catch { setSilent(null) }
    } finally {
      inFlight.current = false
      setBusy(false)
    }
  }

  return <section className="settings-autostart" aria-label="启动设置">
    <div className="settings-form-row">
      <span className="settings-form-label">启动：</span>
      <div className="settings-form-control">
        <label className="settings-checkbox"><input type="checkbox" checked={enabled === true} disabled={!supported || busy || enabled === null} onChange={() => void toggle()} />开机自启</label>
        <SettingsHelp id="autostart-description" label="开机自启">{supported ? '登录电脑后自动启动 Axonkey。' : '请在 Windows 或 macOS 桌面应用中设置。'}</SettingsHelp>
        <span className="settings-autostart-status" role="status">{busy ? '正在同步…' : ''}</span>
      </div>
    </div>
    <div className="settings-form-row">
      <span className="settings-form-label">静默启动：</span>
      <div className="settings-form-control">
        <label className="settings-checkbox"><input type="checkbox" checked={silent === true} disabled={!supported || busy || enabled !== true || silent === null} onChange={() => void toggleSilent()} />开机自启时不显示主窗口</label>
        <SettingsHelp id="silent-start-description" label="静默启动">登录电脑自动启动 Axonkey 时只驻留托盘或菜单栏，不弹出主窗口；从应用列表手动打开仍会显示窗口。需先开启开机自启。</SettingsHelp>
      </div>
    </div>
    {error && <p className="settings-autostart-error" role="alert">{error}<button type="button" disabled={busy} onClick={() => setRefreshVersion(value => value + 1)}>重新读取</button></p>}
  </section>
}
