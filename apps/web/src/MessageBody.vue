<script setup>
import {computed,ref,inject} from 'vue'
import MarkdownText from './MarkdownText.vue'
import {splitAttachments,attachmentUrl} from './attachments.js'
const address=inject('crabotAddress',ref(''))
const url=file=>attachmentUrl(file.url,address.value)
const props=defineProps({text:String})
const body=computed(()=>splitAttachments(props.text||'')),failed=ref(new Set())
function hide(id){failed.value=new Set([...failed.value,id])}
</script>
<template>
 <MarkdownText :text="body.text"/>
 <div v-if="body.files.length" class="message-attachments">
  <div v-for="file in body.files" :key="file.id" class="message-attachment">
   <el-image v-if="!failed.has(file.id)" :src="url(file)+'?preview=true'" :alt="file.name" :preview-src-list="[file.url+'?preview=true']" preview-teleported hide-on-click-modal fit="contain" loading="lazy" @error="hide(file.id)"/>
   <a :href="url(file)" :download="file.name" :title="'下载 '+file.name">↧ {{file.name}}</a>
  </div>
 </div>
</template>
<style scoped>.message-attachments{display:flex;flex-wrap:wrap;gap:10px;margin-top:8px}.message-attachment{display:flex;flex-direction:column;gap:8px;max-width:240px;padding:10px;border:1px solid var(--line);border-radius:10px;color:inherit;text-decoration:none;background:var(--panel)}.message-attachment .el-image{cursor:zoom-in;max-width:220px;max-height:160px;object-fit:contain;border-radius:6px}.message-attachment a{color:inherit;text-decoration:none;font-size:12px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}</style>
