<script setup>
import {ElDialog} from 'element-plus/es/components/dialog/index'
import {Close} from '@element-plus/icons-vue'
import {ref} from 'vue'
const props=defineProps({title:String,eyebrow:String,description:String,busy:Boolean,dirty:Boolean,inline:Boolean})
const emit=defineEmits(['close'])
const confirmClose=ref(false)
function close(){if(props.busy)return;if(props.dirty)confirmClose.value=true;else emit('close')}
</script>
<template>
 <component :is="inline?'section':ElDialog" :model-value="true" class="crabot-dialog" :class="{'inline-shell':inline}" :title="title" :show-close="false" :before-close="close" :close-on-click-modal="false" :close-on-press-escape="!busy" :append-to-body="!inline" align-center>
  <section class="config-panel" :class="{'inline-panel':inline}" :aria-label="title" :role="inline?'region':undefined">
   <header class="config-header"><div><h2>{{title}}</h2><p>{{description}}</p></div><el-button text :icon="Close" :disabled="busy" :aria-label="'关闭'+title" @click="close"/></header>
   <el-alert v-if="confirmClose" type="warning" :closable="false" show-icon title="还有未保存的修改，确定放弃吗？"><el-button @click="confirmClose=false">继续编辑</el-button><el-button type="danger" @click="emit('close')">放弃修改并关闭</el-button></el-alert>
   <slot/>
  </section>
 </component>
</template>
<style scoped>
.inline-shell{min-width:0;min-height:0;height:100%;display:flex;flex-direction:column}
.inline-panel{width:100%;max-width:none;height:100%;max-height:none;border-radius:0;border:0;box-shadow:none;min-width:0}
</style>
