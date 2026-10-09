<script setup>
import {ref,onMounted,computed} from 'vue'
import {ElMessage} from 'element-plus/es/components/message/index'
import {ElMessageBox} from 'element-plus/es/components/message-box/index'
import {vLoading} from 'element-plus/es/components/loading/index'
import 'element-plus/es/components/loading/style/css'
import 'element-plus/es/components/message/style/css'
import 'element-plus/es/components/message-box/style/css'
const props=defineProps({request:{type:Function,required:true}})
const entries=ref([]),directory=ref(''),error=ref(''),loading=ref(false),saving=ref(false)
const selected=ref(null),draft=ref(''),useDefault=ref(true)
const open=computed({get:()=>!!selected.value,set:value=>{if(!value)selected.value=null}})
async function refresh(){
 loading.value=true;error.value=''
 try{const data=await props.request('/v1/config/prompts');entries.value=data.prompts;directory.value=data.directory}
 catch(e){error.value=e.message}finally{loading.value=false}
}
function edit(row){selected.value={...row};draft.value=row.content;useDefault.value=!row.overridden;error.value=''}
async function close(done){
 if(saving.value)return
 if(selected.value&&(useDefault.value===selected.value.overridden||(!useDefault.value&&draft.value!==selected.value.content))){try{await ElMessageBox.confirm('放弃尚未保存的修改？','关闭编辑',{type:'warning',confirmButtonText:'放弃修改',cancelButtonText:'继续编辑'})}catch{return}}
 selected.value=null;done?.()
}
async function save(){
 if(!selected.value)return
 saving.value=true;error.value=''
 try{
  const data=await props.request('/v1/config/prompts',{method:'PUT',headers:{'content-type':'application/json'},body:JSON.stringify({id:selected.value.id,content:draft.value,reset:useDefault.value,expected:selected.value.content})})
  entries.value=data.prompts;directory.value=data.directory;selected.value=null
  ElMessage.success(useDefault.value?'已移除实例覆盖，恢复随版本默认':'实例覆盖已保存，下一次任务生效')
 }catch(e){error.value=e.message}finally{saving.value=false}
}
onMounted(refresh)
</script>
<template>
 <section class="prompt-page">
  <header><div><h2>系统提示词</h2><p>仅影响当前 Crabot 实例 · 修改后下一次任务生效</p></div><el-button :loading="loading" @click="refresh">刷新</el-button></header>
  <p class="prompt-path">{{directory}}</p>
  <el-alert title="默认提示词随版本更新，不复制到实例。保存修改才创建覆盖文件；恢复默认会移除覆盖。也可在此目录创建同名 Markdown 文件。提示词不会取消审批或调度校验，Agent 自定义回复要求及显式 MODEL_SYSTEM_PROMPT 优先。" type="info" :closable="false"/>
  <el-alert v-if="error&&!selected" :title="error" type="error" :closable="false"/>
  <el-table :data="entries" v-loading="loading" class="prompt-table" @row-click="edit">
   <el-table-column label="提示词" min-width="170"><template #default="{row}"><b>{{row.title}}</b><small>{{row.id}}.md</small></template></el-table-column>
   <el-table-column prop="description" label="用途" min-width="280"/>
   <el-table-column label="来源" width="120"><template #default="{row}"><el-tag :type="row.overridden?'primary':'info'" size="small">{{row.overridden?'实例覆盖':'随版本默认'}}</el-tag></template></el-table-column>
   <el-table-column width="80"><template #default="{row}"><el-button link type="primary" @click.stop="edit(row)">编辑</el-button></template></el-table-column>
  </el-table>
  <el-drawer v-model="open" :title="selected?.title||'系统提示词'" size="min(760px, 100vw)" :before-close="close" append-to-body>
   <template v-if="selected">
    <p class="prompt-path">覆盖路径：{{selected.path}}</p><p>{{selected.description}}</p>
    <el-switch v-model="useDefault" active-text="随版本默认" inactive-text="实例覆盖" :disabled="saving" @change="value=>{if(value)draft=selected.default}"/>
    <el-input v-model="draft" type="textarea" :rows="20" :disabled="saving||useDefault" aria-label="系统提示词内容"/>
    <el-alert v-if="error" :title="error" type="error" :closable="false"/>
   </template>
   <template #footer><el-button :disabled="saving" @click="useDefault=true;draft=selected.default">恢复随版本默认</el-button><el-button :disabled="saving" @click="close()">取消</el-button><el-button type="primary" :loading="saving" :disabled="!draft.trim()" @click="save">保存</el-button></template>
  </el-drawer>
 </section>
</template>
<style scoped>
.prompt-page{min-width:0;flex:1;overflow:auto;padding:24px}.prompt-page header{display:flex;align-items:center;justify-content:space-between;gap:16px}.prompt-page h2{margin:0}.prompt-page header p,.prompt-path{color:var(--muted);font-size:12px;overflow-wrap:anywhere}.prompt-table{margin-top:18px}.prompt-table small{display:block;color:var(--muted);font-size:12px}.el-alert{margin-top:12px}
</style>
