<script setup>
import {ref,watch,onBeforeUnmount} from 'vue'
import {List} from '@element-plus/icons-vue'
const props=defineProps({project:String,chat:String,request:Function})
const open=ref(false),rows=ref([]),selected=ref(''),output=ref(''),input=ref(''),error=ref(''),busy=ref(false),privateInput=ref(true)
let timer,epoch=0,cursor=0,loading=false
const base=()=>`/v1/repl/${encodeURIComponent(props.project)}/chats/${encodeURIComponent(props.chat)}/processes`
async function action(op,body){return props.request(`${base()}/${op}`,{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify(body)})}
async function refresh(){
 if(loading||!open.value||!props.project||!props.chat)return
 const version=epoch;loading=true
 try{
  const list=await props.request(base());if(version!==epoch)return;rows.value=list
  if(selected.value){const id=selected.value,row=await action('read',{session_id:id,after:cursor});if(version!==epoch||id!==selected.value)return;cursor=row.next_cursor;output.value=(output.value+(row.truncated?'\n[较早输出已截断]\n':'')+(row.stdout||'')+(row.stderr||'')).slice(-100000)}
 }catch(e){if(version===epoch)error.value=e.message}finally{loading=false}
}
function choose(id){epoch++;selected.value=id;cursor=0;output.value='';input.value='';error.value='';refresh()}
async function send(){
 if(busy.value||!selected.value)return
 const version=epoch,value=input.value;input.value='';busy.value=true;error.value=''
 try{await action('write',{session_id:selected.value,input:value+'\n',private:privateInput.value});if(version===epoch)await refresh()}
 catch(e){if(version===epoch)error.value=e.message}finally{busy.value=false}
}
async function stop(id){try{await action('stop',{session_id:id});await refresh()}catch(e){error.value=e.message}}
function reset(){epoch++;selected.value='';output.value='';input.value='';rows.value=[];error.value='';cursor=0}
watch(()=>[props.project,props.chat],()=>{reset();refresh()})
watch(open,value=>{clearInterval(timer);if(value){refresh();timer=setInterval(refresh,1000)}else reset()})
onBeforeUnmount(()=>{clearInterval(timer);reset()})
</script>
<template>
 <el-button class="secondary toolbar-icon" :disabled="!project||!chat" aria-label="任务" title="任务" @click="open=true"><el-icon><List/></el-icon></el-button>
 <el-drawer v-model="open" title="任务" size="min(720px,96vw)" append-to-body>
  <p class="hint">查看运行输出、完成登录或停止进程。仅本节点进程；远端请在所属节点操作。重置上下文会清理进程。</p>
  <el-table :data="rows" row-key="session_id" highlight-current-row @row-click="row=>choose(row.session_id)">
   <el-table-column prop="agent" label="Agent" width="115"/>
   <el-table-column prop="command" label="命令" show-overflow-tooltip/>
   <el-table-column prop="status" label="状态" width="100"/>
   <el-table-column width="78"><template #default="{row}"><el-button link type="danger" :disabled="row.status!=='running'" @click.stop="stop(row.session_id)">停止</el-button></template></el-table-column>
  </el-table>
  <el-empty v-if="!rows.length" description="暂无进程会话"/>
  <template v-if="selected">
   <pre class="process-output">{{output||'等待输出…'}}</pre>
   <el-checkbox v-model="privateInput">私密输入（不进入聊天，此后输出对模型隐藏）</el-checkbox>
   <el-form @submit.prevent="send"><el-input v-model="input" :type="privateInput?'password':'text'" autocomplete="off" placeholder="输入后按 Enter 发送给进程" :disabled="busy||rows.find(r=>r.session_id===selected)?.status!=='running'"/><el-button @click="send" :loading="busy" :disabled="rows.find(r=>r.session_id===selected)?.status!=='running'">发送到进程</el-button></el-form>
  </template>
  <el-alert v-if="error" :title="error" type="error" :closable="false"/>
 </el-drawer>
</template>
<style scoped>.hint{font-size:12px;color:var(--muted);line-height:1.6}.process-output{white-space:pre-wrap;overflow-wrap:anywhere;max-height:45vh;overflow:auto;background:var(--el-fill-color-light);padding:16px;border-radius:8px;font:12px/1.6 monospace}.el-form{display:flex;gap:8px;margin:10px 0}</style>
