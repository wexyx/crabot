<script setup>
import {ref,onMounted} from 'vue'
const props=defineProps({request:Function,url:String})
const emit=defineEmits(['saved','close'])
const current=ref(null),text=ref(''),error=ref(''),busy=ref(false)
onMounted(async()=>{try{current.value=await props.request(props.url);text.value=current.value.command_allowlist.join('\n')}catch(e){error.value=e.message}})
async function save(){
 busy.value=true;error.value=''
 try{const row=await props.request(props.url,{method:'PUT',headers:{'content-type':'application/json'},body:JSON.stringify({expected_version:current.value.version,command_allowlist:text.value.split('\n').map(s=>s.trim()).filter(Boolean)})});current.value=row;emit('saved',row)}
 catch(e){error.value=e.message}
 finally{busy.value=false}
}
</script>
<template>
 <el-alert type="warning" :closable="false" title="白名单内的命令模式在「帮我批准」模式下无需确认。请仅添加可信命令；脚本、构建、测试也可能执行任意代码。命令以当前系统用户权限执行，可访问工作目录外文件。"/>
 <p class="allowlist-note">每行一条；支持 *（任意字符，含多个参数）和 ?（单个字符），例如 git status *、cat *.md。可执行文件名必须明确，不支持管道、重定向和命令替换。宽泛规则会放行更多操作，请谨慎添加。当前 Crabot 的所有本地 Agent 后续项目任务共用此列表，不用于管理审批或 Codex/Claude 原生工具授权。</p>
 <el-input v-model="text" type="textarea" :rows="14" :disabled="!current||busy" aria-label="自动批准命令白名单"/>
 <el-alert v-if="error" type="error" :title="error" :closable="false"/>
 <div class="allowlist-actions"><el-button :disabled="!current||busy" @click="text=current.default_allowlist.join('\n')">恢复默认列表</el-button><el-button :disabled="busy" @click="text=''">清空</el-button><el-button :disabled="busy" @click="emit('close')">取消</el-button><el-button type="primary" :disabled="!current" :loading="busy" @click="save">保存白名单</el-button></div>
</template>
<style scoped>.allowlist-note{font-size:12px;color:var(--muted);margin:12px 0;line-height:1.7}.allowlist-actions{display:flex;justify-content:flex-end;gap:8px;margin-top:16px;flex-wrap:wrap}</style>
