<script setup lang="ts">
const emit = defineEmits<{ accept: []; decline: [] }>()
</script>

<template>
  <transition name="overlay-fade">
    <div class="privacy-overlay" @click.self>
      <div class="privacy-dialog">
        <div class="privacy-title">🔍 增强好奇心 - 隐私授权</div>
        <div class="privacy-body">
          <p>增强好奇心模式会定期截取您的桌面屏幕，让桌宠了解您正在做什么。</p>
          <p><strong>隐私保护措施：</strong></p>
          <ul>
            <li>截图数据仅在本地处理，不会上传或存储原始截图</li>
            <li>仅将 AI 分析结果的文本摘要记入主人印象</li>
            <li>您可随时在设置中关闭或清除已收集的印象数据</li>
          </ul>
          <p class="privacy-warning">请确认您同意此功能的使用。</p>
        </div>
        <div class="privacy-buttons">
          <button class="btn btn-privacy-accept" @click.stop="emit('accept')">同意并开启</button>
          <button class="btn btn-privacy-decline" @click.stop="emit('decline')">拒绝</button>
        </div>
      </div>
    </div>
  </transition>
</template>

<style scoped>
.privacy-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.6);
  z-index: 2000;
  display: flex;
  align-items: center;
  justify-content: center;
}

.privacy-dialog {
  background: white;
  border-radius: 16px;
  padding: 24px;
  max-width: 320px;
  width: 90%;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.2);
}

.privacy-title {
  font-size: 15px;
  font-weight: 700;
  margin-bottom: 12px;
  color: #333;
}

.privacy-body {
  font-size: 12px;
  line-height: 1.7;
  color: #555;
}

.privacy-body p {
  margin: 6px 0;
}

.privacy-body ul {
  margin: 6px 0;
  padding-left: 18px;
}

.privacy-body li {
  margin: 3px 0;
}

.privacy-warning {
  color: #f44336;
  font-weight: 600;
  margin-top: 10px;
}

.privacy-buttons {
  display: flex;
  gap: 8px;
  margin-top: 16px;
}

.btn-privacy-accept {
  flex: 1;
  background: linear-gradient(135deg, #4caf50, #8bc34a);
  color: white;
  border: none;
  padding: 10px 16px;
  border-radius: 10px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
}

.btn-privacy-decline {
  flex: 1;
  background: #f5f5f5;
  color: #666;
  border: 1px solid #ddd;
  padding: 10px 16px;
  border-radius: 10px;
  font-size: 13px;
  cursor: pointer;
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
