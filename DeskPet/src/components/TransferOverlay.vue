<script setup lang="ts">
import { personalityKeys, personalityLabels, type TransferResult } from '../composables/useTransfer'

defineProps<{
  phase: 'idle' | 'animating' | 'revealing' | 'done'
  progress: number
  result: TransferResult | null
  petEmoji: string
  particles: Array<{ x: number; y: number; size: number; opacity: number; hue: number }>
}>()

const emit = defineEmits<{ finish: [] }>()
</script>

<template>
  <transition name="overlay-fade">
    <div
      v-if="phase === 'animating' || phase === 'revealing'"
      class="transfer-overlay"
      @click.self
    >
      <!-- Animation Phase -->
      <div v-if="phase === 'animating'" class="transfer-anim-container">
        <div class="transfer-core">
          <div
            class="transfer-ring"
            :style="{ transform: `scale(${0.5 + progress * 0.5})`, opacity: 1 - progress * 0.3 }"
          >
            <div class="transfer-inner-ring"></div>
          </div>
          <div class="transfer-pet-icon">{{ petEmoji }}</div>
          <svg
            v-for="(p, i) in particles"
            :key="i"
            class="transfer-particle"
            :style="{ left: p.x + '%', top: p.y + '%', opacity: p.opacity * progress }"
            width="6"
            height="6"
            viewBox="0 0 6 6"
          >
            <circle cx="3" cy="3" :r="p.size" :fill="`hsl(${p.hue}, 70%, 65%)`" />
          </svg>
        </div>
        <div class="transfer-text">
          <span class="transfer-label">灵魂正在传送</span>
          <div class="transfer-progress-bar">
            <div class="transfer-progress-fill" :style="{ width: (progress * 100) + '%' }"></div>
          </div>
          <span class="transfer-percent">{{ Math.round(progress * 100) }}%</span>
        </div>
      </div>

      <!-- Reveal Phase -->
      <div v-if="phase === 'revealing' && result" class="transfer-reveal">
        <div class="reveal-title">传送完成</div>
        <div class="reveal-generation">第 {{ result.generation }} 代灵魂</div>

        <div class="reveal-signatures">
          <div class="sig-item sig-old">
            <div class="sig-label">原体签名</div>
            <div class="sig-value sig-faded">{{ result.oldSignature.slice(0, 8) }}...</div>
            <div class="sig-status">已消逝</div>
          </div>
          <div class="sig-arrow">→</div>
          <div class="sig-item sig-new">
            <div class="sig-label">新体签名</div>
            <div class="sig-value">{{ result.newSignature.slice(0, 8) }}...</div>
            <div class="sig-status sig-active">已激活</div>
          </div>
        </div>

        <div class="reveal-perturbation">
          <div class="perturb-title">人格微偏</div>
          <div class="perturb-grid">
            <div v-for="key in personalityKeys" :key="key" class="perturb-item">
              <span class="perturb-label">{{ personalityLabels[key] || key }}</span>
              <div class="perturb-bar-container">
                <div class="perturb-bar-bg">
                  <div
                    class="perturb-bar-fill"
                    :class="{
                      'perturb-positive': (result.perturbation[key] || 0) > 0,
                      'perturb-negative': (result.perturbation[key] || 0) < 0,
                    }"
                    :style="{ width: Math.min(Math.abs(result.perturbation[key] || 0) * 1000, 100) + '%' }"
                  ></div>
                </div>
              </div>
              <span
                class="perturb-value"
                :class="{
                  'pv-pos': (result.perturbation[key] || 0) > 0,
                  'pv-neg': (result.perturbation[key] || 0) < 0,
                }"
              >
                {{ (result.perturbation[key] || 0) > 0 ? '+' : '' }}{{ ((result.perturbation[key] || 0) * 100).toFixed(1) }}%
              </span>
            </div>
          </div>
        </div>

        <button @click.stop="emit('finish')" class="btn btn-transfer-complete">确认 · 继续陪伴</button>
      </div>
    </div>
  </transition>
</template>

<style scoped>
.transfer-overlay {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  background: rgba(10, 5, 20, 0.95);
  z-index: 2000;
  display: flex;
  align-items: center;
  justify-content: center;
  color: white;
  font-family: 'Segoe UI', system-ui, sans-serif;
}

.transfer-anim-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 32px;
}

.transfer-core {
  position: relative;
  width: 200px;
  height: 200px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.transfer-ring {
  position: absolute;
  width: 160px;
  height: 160px;
  border-radius: 50%;
  border: 2px solid rgba(124, 77, 255, 0.4);
  animation: transferPulse 1.5s ease-in-out infinite;
}

.transfer-inner-ring {
  width: 100%;
  height: 100%;
  border-radius: 50%;
  border: 1px solid rgba(68, 138, 255, 0.3);
  animation: transferSpin 3s linear infinite;
}

@keyframes transferPulse {
  0%, 100% { transform: scale(1); opacity: 0.6; }
  50% { transform: scale(1.1); opacity: 1; }
}

@keyframes transferSpin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.transfer-pet-icon {
  font-size: 48px;
  z-index: 10;
  animation: transferFloat 2s ease-in-out infinite;
}

@keyframes transferFloat {
  0%, 100% { transform: translateY(0); }
  50% { transform: translateY(-8px); }
}

.transfer-particle {
  position: absolute;
  pointer-events: none;
  animation: transferParticleFade 2s ease-out infinite;
}

@keyframes transferParticleFade {
  0% { opacity: 0.8; transform: scale(1); }
  100% { opacity: 0.2; transform: scale(0.5); }
}

.transfer-text {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
}

.transfer-label {
  font-size: 18px;
  font-weight: 700;
  background: linear-gradient(135deg, #7c4dff, #448aff, #ff6b9d);
  background-size: 200% 200%;
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
  background-clip: text;
  animation: transferGradient 3s ease infinite;
}

@keyframes transferGradient {
  0% { background-position: 0% 50%; }
  50% { background-position: 100% 50%; }
  100% { background-position: 0% 50%; }
}

.transfer-progress-bar {
  width: 200px;
  height: 4px;
  background: rgba(255, 255, 255, 0.1);
  border-radius: 2px;
  overflow: hidden;
}

.transfer-progress-fill {
  height: 100%;
  background: linear-gradient(90deg, #7c4dff, #448aff, #ff6b9d);
  border-radius: 2px;
  transition: width 0.1s linear;
}

.transfer-percent {
  font-size: 12px;
  color: rgba(255, 255, 255, 0.5);
}

/* Reveal Phase */
.transfer-reveal {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 16px;
  padding: 24px;
  max-width: 320px;
  animation: revealFadeIn 0.6s ease;
}

@keyframes revealFadeIn {
  from { opacity: 0; transform: translateY(10px); }
  to { opacity: 1; transform: translateY(0); }
}

.reveal-title {
  font-size: 22px;
  font-weight: 800;
  background: linear-gradient(135deg, #7c4dff, #ff6b9d);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
  background-clip: text;
}

.reveal-generation {
  font-size: 14px;
  color: rgba(255, 255, 255, 0.6);
}

.reveal-signatures {
  display: flex;
  align-items: center;
  gap: 12px;
  margin: 8px 0;
}

.sig-item {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  padding: 10px 14px;
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(255, 255, 255, 0.1);
}

.sig-old {
  opacity: 0.4;
}

.sig-new {
  border-color: rgba(124, 77, 255, 0.3);
  background: rgba(124, 77, 255, 0.08);
}

.sig-label {
  font-size: 10px;
  color: rgba(255, 255, 255, 0.5);
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.sig-value {
  font-size: 12px;
  font-weight: 600;
  font-family: 'Courier New', monospace;
}

.sig-faded {
  text-decoration: line-through;
  color: rgba(255, 255, 255, 0.3);
}

.sig-status {
  font-size: 9px;
  padding: 2px 6px;
  border-radius: 4px;
  background: rgba(255, 255, 255, 0.08);
}

.sig-active {
  color: #7c4dff;
  background: rgba(124, 77, 255, 0.15);
}

.sig-arrow {
  font-size: 18px;
  color: rgba(255, 255, 255, 0.3);
}

.reveal-perturbation {
  width: 100%;
  margin-top: 4px;
}

.perturb-title {
  font-size: 12px;
  color: rgba(255, 255, 255, 0.5);
  text-transform: uppercase;
  letter-spacing: 0.5px;
  margin-bottom: 8px;
  text-align: center;
}

.perturb-grid {
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.perturb-item {
  display: flex;
  align-items: center;
  gap: 8px;
}

.perturb-label {
  width: 28px;
  text-align: right;
  font-size: 11px;
  color: rgba(255, 255, 255, 0.6);
  flex-shrink: 0;
}

.perturb-bar-container {
  flex: 1;
}

.perturb-bar-bg {
  height: 6px;
  background: rgba(255, 255, 255, 0.08);
  border-radius: 3px;
  overflow: hidden;
}

.perturb-bar-fill {
  height: 100%;
  border-radius: 3px;
  transition: width 0.8s ease;
}

.perturb-positive {
  background: linear-gradient(90deg, rgba(124, 77, 255, 0.6), rgba(124, 77, 255, 1));
}

.perturb-negative {
  background: linear-gradient(90deg, rgba(255, 107, 157, 0.6), rgba(255, 107, 157, 1));
}

.perturb-value {
  width: 48px;
  font-size: 11px;
  font-weight: 600;
  font-family: 'Courier New', monospace;
  flex-shrink: 0;
}

.pv-pos {
  color: #b388ff;
}

.pv-neg {
  color: #ff6b9d;
}

.btn-transfer-complete {
  margin-top: 8px;
  padding: 10px 24px;
  background: linear-gradient(135deg, #7c4dff, #448aff);
  color: white;
  border: none;
  border-radius: 12px;
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  transition: transform 0.15s, box-shadow 0.15s;
}

.btn-transfer-complete:hover {
  transform: translateY(-2px);
  box-shadow: 0 4px 16px rgba(124, 77, 255, 0.4);
}

.overlay-fade-enter-active,
.overlay-fade-leave-active {
  transition: opacity 0.2s ease;
}

.overlay-fade-enter-from,
.overlay-fade-leave-to {
  opacity: 0;
}
</style>
