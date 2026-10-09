<script setup>
import {ref,computed,onMounted,onBeforeUnmount} from 'vue'
import ApprovalCard from './ApprovalCard.vue'
import ChatMessages from './ChatMessages.vue'
import AttachmentPicker from './AttachmentPicker.vue'
import {attachmentMessage} from './attachments.js'
import {conversationView} from './conversation-view.js'
const props=defineProps({agent:Object,project:String,request:Function,stream:Function})
defineEmits(['close'])
const attachments=ref([]),attachmentPicker=ref(null),uploading=ref(false)
const hasMore=ref(false)
async function older(){const result=await props.request(base()+'/chats/'+group.value+'/history?before='+(rows.value[0]?.seq||0));if(!alive)return;const ids=new Set(rows.value.map(r=>r.seq));rows.value=[...result.events.filter(r=>!ids.has(r.seq)),...rows.value];hasMore.value=result.has_more}
const rows=ref([]),text=ref(''),error=ref(''),busy=ref(false),running=ref(false),group=ref(''),run=ref(''),permissions=ref([]),optimistic=ref(null)
const messages=computed(()=>{const result=conversationView(rows.value);if(optimistic.value)result.push(optimistic.value);return result})
let live,alive=true,timer
const post=body=>({method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify(body)})
const base=()=>'/v1/repl/'+props.project
function add(event){if(!alive||rows.value.some(r=>r.seq===event.seq))return;rows.value.push(event);rows.value.sort((a,b)=>a.seq-b.seq);if(['user','message.created'].includes(event.type))optimistic.value=null;if(['completed','failed','agent.done','agent.error','task.interrupted'].includes(event.type))running.value=false}
function onEnter(event){if(!event.shiftKey&&!event.isComposing){event.preventDefault();send()}}
async function send(){
 const draft=text.value,content=attachmentMessage(draft,attachments.value);if(!group.value||!content.trim()||busy.value||uploading.value||running.value)return
 text.value='';busy.value=true;running.value=true;error.value='';optimistic.value={seq:'pending',type:'user',text:content,label:'你',timestamp:Date.now()}
 try{const result=await props.request(base()+'/groups/'+group.value+'/messages',post({content}));run.value=result.id;attachments.value=[]}
 catch(e){error.value=e.message;running.value=false;optimistic.value=null;if(!text.value)text.value=draft}
 finally{busy.value=false}
}
async function decide(id,allow,conversation=false){try{await props.request('/v1/workspace/approvals/'+id,post({allow,conversation}));permissions.value=permissions.value.filter(p=>p.id!==id)}catch(e){error.value=e.message}}
async function stop(){try{await props.request('/v1/sessions/'+run.value+'/interrupt',post({}));}catch(e){error.value=e.message}}
onMounted(async()=>{
 try{
  const value=await props.request(base()+'/agent-tests',post({path:props.agent.path||[props.agent.id]}));if(!alive)return;group.value=value.key
  const history=await props.request(base()+'/chats/'+group.value+'/history');if(!alive)return;rows.value=history.events;hasMore.value=history.has_more;run.value=history.active_run||'';running.value=!!run.value
  const opened=await props.stream(base()+'/chats/'+group.value+'/events?after='+(rows.value.at(-1)?.seq||0),add,e=>{if(alive)error.value=e.message})
  if(!alive){opened.close();return}live=opened
  timer=setInterval(async()=>{try{const result=await props.request('/v1/workspace/approvals');if(alive)permissions.value=result.requests.filter(p=>p.conversation_id===props.project+':'+group.value)}catch(e){if(alive)error.value=e.message}},1500)
 }catch(e){if(alive)error.value=e.message}
})
onBeforeUnmount(()=>{alive=false;live?.close();clearInterval(timer)})
</script>
<template>
 <section class="chat agent-test-chat">
  <header class="chat-header"><div><span class="eyebrow">AGENT PLAYGROUND</span><h2>{{agent.name||agent.id}}</h2><p>{{agent.provider||'组合协作'}} <span>·</span> 测试会话独立保存，不影响项目聊天</p></div><el-button type="default" native-type="button" class="secondary" @click="$emit('close')" aria-label="关闭测试聊天">关闭测试</el-button></header>
  <p v-if="error" class="config-alert danger" role="alert">{{error}}</p>
  <ChatMessages :has-more="hasMore" :load-older="older" @history-error="error=$event" :messages="messages" :running="running" :empty-title="'测试 '+(agent.name||agent.id)" empty-description="发送一条消息，验证角色、模型与工具表现。测试记录会自动保留。"/>
  <div class="composer-feedback"><ApprovalCard v-for="p in permissions" :key="p.id" :item="p" workspace @conversation="decide(p.id,true,true)" @decide="allow=>decide(p.id,allow)"/></div>
  <div class="test-input-area" @dragover.prevent @drop="attachmentPicker?.drop($event)" @paste="attachmentPicker?.paste($event)"><el-form class="test-composer" @submit.prevent="send" label-position="top"><el-input type="textarea" v-model="text" aria-label="测试消息" placeholder="发送消息验证 Agent…" @keydown.enter="onEnter"/><AttachmentPicker ref="attachmentPicker" v-model="attachments" :request="request" :disabled="!group||busy||running" @uploading="uploading=$event" @error="error=$event"/><el-button type="default" native-type="button" v-if="running&&run" class="secondary" @click="stop">停止</el-button><el-button type="primary" native-type="button" @click="send" :loading="busy" :disabled="!group||busy||running||uploading||(!text.trim()&&!attachments.length)">发送</el-button></el-form><p class="test-input-hint">{{running?'Agent 正在处理，可随时停止。':'Enter 发送 · Shift + Enter 换行'}}<span>独立测试会话</span></p></div>
 </section>
</template>
<style scoped>
.agent-test-chat{min-width:0;min-height:0;flex:1;background:var(--bg)}.chat-header{padding:22px 30px;background:var(--panel)}
.chat-header h2{margin:6px 0;font-size:20px}.chat-header p{margin:0;font-size:12px;color:var(--muted)}.chat-header p span{padding:0 6px}
.test-input-area{padding:14px 24px 20px;width:100%;max-width:980px;box-sizing:border-box;align-self:center}
.test-composer{position:relative;flex-wrap:wrap;display:flex;align-items:flex-end;gap:12px;padding:14px;border:1px solid var(--line);border-radius:16px;background:var(--panel);box-shadow:0 8px 28px #00000010}
.test-composer:focus-within{border-color:var(--accent)}.test-composer textarea{flex:1;resize:vertical;min-height:64px;max-height:240px;background:transparent;border:0;box-shadow:none;outline:none;padding:6px}
.test-input-hint{display:flex;justify-content:space-between;gap:12px;margin:10px 4px 0;color:var(--muted);font-size:11px}
.approval{max-height:180px;overflow:auto;margin:10px 24px}.approval code,.approval small{display:block;margin:10px 0;overflow-wrap:anywhere}.config-alert{margin:12px}
@media(max-width:600px){.chat-header{padding:16px}.test-input-area{padding:12px}.test-input-hint span{display:none}}
</style>
