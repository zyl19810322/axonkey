import { invoke } from '@tauri-apps/api/core'
import { useEffect, useState } from 'react'
import {
  audioGainMax,
  audioGainMin,
  audioSettingsStorageKey,
  getStoredAudioGain,
  getStoredSmartGain,
} from '../appConfig'
import type { Platform } from '../appTypes'
import { logError, logInfo } from '../runtimeLogging'

type UseAudioControlsOptions = {
  platform: Platform
  nativeRuntime: boolean
  onToast: (message: string) => void
}

export function useAudioControls({ platform, nativeRuntime, onToast }: UseAudioControlsOptions) {
  const [audioGain, setAudioGain] = useState(getStoredAudioGain)
  const [smartGain, setSmartGain] = useState(getStoredSmartGain)
  const [gainError, setGainError] = useState('')

  useEffect(() => {
    window.localStorage.setItem(audioSettingsStorageKey, JSON.stringify({ gain: audioGain, smartGain }))
  }, [audioGain, smartGain])

  useEffect(() => {
    if (platform === 'unsupported' || !nativeRuntime) return
    void invoke('set_audio_gain', { gain: audioGain }).catch((error) => {
      logError('Failed to initialize audio gain', error)
      setGainError(`音频增益未生效：${String(error)}`)
    })
    void invoke('set_smart_gain_enabled', { enabled: smartGain }).catch((error) => {
      logError('Failed to initialize smart gain', error)
      setGainError(`智能增益未生效：${String(error)}`)
    })
  }, [nativeRuntime, platform])

  const updateAudioGain = (value: number) => {
    const next = Math.max(audioGainMin, Math.min(audioGainMax, Math.round(value)))
    setAudioGain(next)
    setGainError('')
    if (platform === 'unsupported' || !nativeRuntime) return
    logInfo(`Updating audio gain from frontend: ${next} dB`)
    void invoke('set_audio_gain', { gain: next }).catch((error) => {
      logError('Failed to update audio gain', error)
      setGainError(`音频增益未生效：${String(error)}`)
      onToast(`音频增益未生效：${String(error)}`)
      window.setTimeout(() => onToast(''), 2600)
    })
  }

  const updateSmartGain = (enabled: boolean) => {
    setSmartGain(enabled)
    setGainError('')
    if (platform === 'unsupported' || !nativeRuntime) return
    logInfo(`Updating smart gain from frontend: ${enabled}`)
    void invoke('set_smart_gain_enabled', { enabled }).catch((error) => {
      logError('Failed to update smart gain', error)
      setGainError(`智能增益未生效：${String(error)}`)
      onToast(`智能增益未生效：${String(error)}`)
      window.setTimeout(() => onToast(''), 2600)
    })
  }

  return { audioGain, gainError, updateAudioGain, smartGain, updateSmartGain }
}
