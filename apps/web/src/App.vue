<script setup>
import {Setting,Sunny,Moon,Plus,Refresh,Menu,ChatDotRound,User,Connection} from '@element-plus/icons-vue'
import {messageId} from './message-id.js'
import {toolEventSummary} from './tool-summary.js'
import { ref, computed, watch, provide, onBeforeUnmount, onMounted } from 'vue'
import {useRoute,useRouter} from 'vue-router'
import { createAgentConnection, normalizeAgentUrl } from './agent-api.js'
import ApprovalCard from './ApprovalCard.vue'
import PermissionControl from './PermissionControl.vue'
import ProcessSessions from './ProcessSessions.vue'
import CommandAllowlist from './CommandAllowlist.vue'
import SystemPrompts from './SystemPrompts.vue'
import DocumentLibrary from './DocumentLibrary.vue'
import CapabilityLibrary from './CapabilityLibrary.vue'
import GroupConfiguration from './GroupConfiguration.vue'
import AgentManagement from './AgentManagement.vue'
import ChatMenu from './ChatMenu.vue'
import NetworkConfiguration from './NetworkConfiguration.vue'
import {commandSuggestions,mentionHint,validateGroupCommand} from './group-commands.js'
import {applyMention,highlighted,mentionAt,mentionCandidates,mentionKey,moveHighlight} from './mentions.js'
import MarkdownText from './MarkdownText.vue'
import ChatMessages from './ChatMessages.vue'
import AttachmentPicker from './AttachmentPicker.vue'
import {attachmentMessage} from './attachments.js'
import { conversationView } from './conversation-view.js'
const route=useRoute(),router=useRouter()
let routeVersion=0,loadedProject=''
const theme=ref(localStorage.getItem('crabot.theme')||'dark')
watch(theme,value=>{document.documentElement.dataset.theme=value;document.documentElement.classList.toggle('dark',value==='dark');localStorage.setItem('crabot.theme',value)},{immediate:true})
const address=ref(location.origin), connected=ref(false), error=ref(''), busy=ref(false)
const nodeInfo=ref(null), connectedAddress=ref('')
provide('crabotAddress',connectedAddress)
const connectionHost=computed(()=>{try{return new URL(connectedAddress.value).host}catch{return '未连接'}})
const projects=ref([]), project=ref(''), groups=ref([]), sessions=ref([]), selected=ref(''), session=ref(''), records=ref([]), text=ref('')
const attachments=ref([]),attachmentPicker=ref(null),uploading=ref(false)
watch([project,selected],()=>{attachments.value=[];uploading.value=false})
const agentsRefreshing=ref(false)
async function refreshAgents(){if(agentsRefreshing.value)return;agentsRefreshing.value=true;try{await perform(()=>refresh(true))}finally{agentsRefreshing.value=false}}
const networkOpen=ref(false)
const newGroupOpen=ref(false),agentsOpen=ref(false),managedAgents=ref([]),pendingMessage=ref(null)
const emptyGroup={key:'',version:0,body:{name:'',policy:{mode:'chat',relay_strategy:'manual',members:[],leader:null,rounds:1,instructions:''}}}
const allowlistOpen=computed(()=>route.query.panel==='allowlist')
const promptsOpen=computed(()=>route.query.panel==='prompts'&&tab.value==='management')
const docsOpen=computed(()=>route.query.panel==='docs'&&tab.value==='projects')
const allowlistVersion=ref(0)
const skillsOpen=ref(false),toolsOpen=ref(false),commandReply=ref(''),membersOpen=ref(false),tab=ref('projects')
const selectedGroup=computed(()=>groups.value.find(g=>g.key===selected.value))
const suggestions=computed(()=>selected.value?commandSuggestions(text.value).filter(c=>selected.value!=='admin'||c.name==='/new'):[])
// `@name` addresses one member of this group instead of the whole roster.
const groupMembers=computed(()=>selectedGroup.value?.body?.policy?.members?.map(m=>m.path?.[m.path.length-1]).filter(Boolean)||[])
// The picker is positioned by the `@` being typed, so it opens above the composer
// instead of pushing the text area around.
const mentionAtCursor=computed(()=>mentionAt(text.value))
const mentionIndex=ref(0),dismissedAt=ref(-1)
const mentionOpen=computed(()=>mentionAtCursor.value&&mentionAtCursor.value.start!==dismissedAt.value)
const mentionChoices=computed(()=>mentionOpen.value?mentionCandidates(text.value,groupMembers.value):[])
const mentionHintText=computed(()=>mentionHint(groupMembers.value))
// Filtering and moving between groups both restart the highlight at the top, and a
// `@` that was deleted entirely forgets its dismissal so the next one reopens.
watch(mentionChoices,()=>{mentionIndex.value=0})
watch(mentionAtCursor,value=>{if(!value&&dismissedAt.value>=0)dismissedAt.value=-1})
const apiRequest=(...args)=>connection.request(...args)
const apiStream=(...args)=>connection.stream(...args)
function assignChanged(target,value){if(JSON.stringify(target.value)!==JSON.stringify(value))target.value=value}
function insertCommand(value){text.value=value;membersOpen.value=false;document.querySelector('textarea[aria-label="消息"]')?.focus()}
function insertMention(name){text.value=applyMention(text.value,name);dismissedAt.value=-1;document.querySelector('textarea[aria-label="消息"]')?.focus()}
async function selectTab(value){await router.push({name:value==='management'?'management':'project',params:value==='projects'&&groups.value.length?{group:groups.value[0].key}:{}})}
const settings=ref(false), mobileList=ref(false), topicId=ref('')
const hasMore=ref(false)
const approvals=ref([]), paths=ref([]), topics=ref([]), running=ref(false)
const visiblePaths=computed(()=>paths.value.filter(item=>item.conversation_id ? item.conversation_id===project.value+':'+selected.value : selected.value==='admin'))
const progress=computed(()=>{const row=[...records.value].reverse().find(r=>['agent.progress','tool_started','agent.tool.started'].includes(r.type));if(!running.value||!row)return '';return row.type==='agent.progress'?'执行中 · '+(row.agent==='default'?'默认 Agent':row.agent):'正在调用工具 · '+toolEventSummary(row)})
const messages=computed(()=>{const rows=conversationView(records.value);if(pendingMessage.value?.chat===selected.value)rows.push({seq:pendingMessage.value.id,type:'user',text:pendingMessage.value.content,timestamp:pendingMessage.value.timestamp,label:'你'});return rows})
let connection, stream, timer, epoch=0,refreshing=false,refreshVersion=0
const post=body=>({method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify(body)})
const base=()=>`/v1/admin-agent/${project.value}`
const title=computed(()=>selected.value==='admin'?'管理':groups.value.find(g=>g.key===selected.value)?.body.name || '群聊')
async function perform(fn){try{error.value='';await fn()}catch(e){if(e.name!=='AbortError')error.value=e.message}}
function close(){pendingMessage.value=null;loadedProject='';routeVersion++;epoch++;stream?.close();stream=null;connection?.close();connection=null;clearInterval(timer);connected.value=false;nodeInfo.value=null;connectedAddress.value='';records.value=[]}
async function connect(){
  close()
  connection=createAgentConnection(normalizeAgentUrl(address.value))
  const current=connection
  try {
    const [info,profile]=await Promise.all([current.request('/v1/repl'),current.request('/v1/agent')])
    if(current!==connection)return
    nodeInfo.value=profile;connectedAddress.value=normalizeAgentUrl(address.value);projects.value=info.projects;groups.value=info.collaboration_projects||[];project.value=projects.value[0]?.id || '';connected.value=true
    if(project.value)await restoreRoute()
    settings.value=false
    timer=setInterval(()=>{if(!document.hidden)refresh().catch(e=>{if(e.name!=='AbortError')error.value=e.message})},3000)
  }catch(e){close();throw e}
}
async function refresh(force=false){
  const c=connection,p=project.value
  if(!c||!p||(refreshing&&!force))return
  const refreshId=++refreshVersion
  refreshing=true
  try{
  const [g,s,a,w,d]=await Promise.all([c.request('/v1/repl'),c.request(`/v1/admin-agent/${p}/sessions`),c.request(`/v1/admin-agent/${p}/approvals`),c.request('/v1/workspace/approvals'),c.request(`/v1/repl/${p}/agents/candidates`)])
  if(c!==connection||p!==project.value||refreshId!==refreshVersion)return
  assignChanged(managedAgents,d.agents);assignChanged(groups,g.collaboration_projects||[]);assignChanged(sessions,s);assignChanged(approvals,a);assignChanged(paths,w.requests)
  }finally{if(refreshId===refreshVersion)refreshing=false}
}
async function choose(id){await router.push({name:id==='admin'?'management':'project',params:id&&id!=='admin'?{group:id}:{}})}
function inspectAgent(id){mobileList.value=false;router.push({path:route.path,query:{panel:"agents",agent:id}})}
function openPanel(panel){router.push({path:route.path,query:{panel}})}
function closePanel(){router.replace({path:route.path})}
async function groupCreated(group){groups.value=[...groups.value.filter(g=>g.key!==group.key),{...group,namespace_id:project.value}];await router.replace({name:'project',params:{group:group.key}})}
async function restoreRoute(){
 const version=++routeVersion
 const admin=route.name==='management'||route.name==='legacy-management'
 if(admin&&route.query.panel==='docs'){await router.replace({path:route.path});return}
 const group=groups.value.find(g=>g.key===route.params.group)
 if(!admin&&!group&&groups.value.length){await router.replace({name:'project',params:{group:groups.value[0].key},query:route.query});return}
 const p=admin?(projects.value.find(p=>p.id===route.params.project)?.id||projects.value[0]?.id):(group?.namespace_id||projects.value[0]?.id)
 if(!p)return
 const changed=loadedProject!==p
 project.value=p
 if(changed){epoch++;stream?.close();records.value=[];await refresh(true);if(version!==routeVersion)return;loadedProject=p}
 const target=admin?'admin':group?.key||''
 tab.value=admin?'management':'projects'
 if(changed||selected.value!==target)await openSelection(target)
 if(version!==routeVersion)return
 agentsOpen.value=route.query.panel==='agents'&&admin;
 networkOpen.value=route.query.panel==='connections'&&admin;
 newGroupOpen.value=route.query.panel==='new-group';skillsOpen.value=route.query.panel==='skills';toolsOpen.value=route.query.panel==='tools';membersOpen.value=route.query.panel==='group'&&!!selectedGroup.value
}
watch(()=>route.fullPath,()=>{if(connected.value)perform(restoreRoute)})
function add(event){
  const key=event.seq
  if(records.value.some(r=>r.seq===key))return
  if(pendingMessage.value&&['user','message.created'].includes(event.type)&&(event.content??event.text??event.payload?.content)===pendingMessage.value.content)pendingMessage.value=null
  records.value.push(event);records.value.sort((a,b)=>a.seq-b.seq)
  if(event.type==='stream_error'){error.value=event.message;stream?.close();return}
  if(['user','message.created'].includes(event.type))running.value=true
  if(['completed','failed','agent.done','agent.error','task.interrupted'].includes(event.type))running.value=false
}
async function open(id){
  const version=++epoch,c=connection,p=project.value,admin=selected.value==='admin'
  stream?.close();stream=null;records.value=[];mobileList.value=false
  topicId.value=selected.value;session.value=admin?id:''
  const logPath=`/v1/repl/${p}/chats/${encodeURIComponent(selected.value)}`
  const result=await c.request(logPath+'/history')
  if(version!==epoch||c!==connection)return
  result.events.forEach(add);hasMore.value=result.has_more
  if(admin){const state=await c.request(`/v1/admin-agent/${p}/sessions/${id}`);if(version!==epoch)return;running.value=state.status==='running';state.events.filter(e=>e.seq>(records.value.at(-1)?.seq||0)).forEach(add)}
  else {session.value=result.active_run||'';running.value=!!result.active_run}
  const after=records.value.at(-1)?.seq||0
  const path=admin?`/v1/admin-agent/${p}/sessions/${id}/events`:logPath+'/events'
  const next=await c.stream(path+`?after=${after}`,event=>{if(version===epoch)add(event)},e=>{if(version===epoch)error.value=e.message})
  if(version!==epoch)next.close();else stream=next
}
async function older(){
  const version=epoch
  const result=await connection.request(`/v1/repl/${project.value}/chats/${encodeURIComponent(selected.value)}/history?before=${records.value[0]?.seq||0}`)
  if(version!==epoch)return
  const keys=new Set(records.value.map(e=>e.seq));records.value=[...result.events.filter(e=>!keys.has(e.seq)),...records.value];hasMore.value=result.has_more
}
async function openSelection(id){
  pendingMessage.value=null;commandReply.value='';membersOpen.value=false;selected.value=id;session.value='';records.value=[];topics.value=[];epoch++;stream?.close();running.value=false
  if(!id){hasMore.value=false;return}
  if(id!=='admin'){await open(id);return}
  const latest=[...(id==='admin'?sessions.value:topics.value)].sort((a,b)=>(b.updated_at||0)-(a.updated_at||0))[0]
  if(latest)await open(latest.id)
}
async function newChat(){
  if(selected.value==='admin'){const row=await connection.request(base()+'/sessions',post({}));await open(row.id)}
  else await open(selected.value)
}
async function groupCommand(command){
  const invalid=validateGroupCommand(command);if(invalid)throw new Error(invalid)
  let result
  try{result=await connection.request(`/v1/repl/${project.value}/groups/${selected.value}/commands`,post({command}))}catch(e){if(e.message.includes('404'))throw new Error('当前运行的服务未提供群命令接口，请重启 crabot 加载新版本，再重新打开 Web。');throw e}
  commandReply.value=result.message||'已处理';await refresh(true)
}
async function resetContext(){
  if(busy.value||running.value||!project.value)return
  busy.value=true
  try{
    if(selected.value==='admin'){
      if(!session.value)await newChat()
      const result=await connection.request(base()+`/sessions/${session.value}/new`,post({}))
      commandReply.value=result.message
    }else{
      await groupCommand('/new')
      session.value=''
    }
  }finally{busy.value=false}
}
async function send(){
  const draft=text.value,content=attachmentMessage(draft,attachments.value),chat=selected.value,p=project.value,c=connection
  const isCommand=content.trim()==='/new'||(chat!=='admin'&&content.trim().startsWith('/'))
  if(!content.trim()||busy.value||uploading.value)return
  const wasRunning=running.value
  if(isCommand&&attachments.value.length)throw new Error('附件请与普通消息一起发送，不能附在命令上')
  const same=()=>connection===c&&selected.value===chat&&project.value===p
  busy.value=true;text.value=''
  try{
    if(!isCommand){pendingMessage.value={id:messageId(),chat,content,timestamp:Date.now()};running.value=true}
    if(content.trim()==='/new'){
      if(chat==='admin'){
        if(!session.value)await newChat()
        const result=await c.request(base()+`/sessions/${session.value}/new`,post({}))
        commandReply.value=result.message
      }else{await groupCommand('/new');session.value=''}
      return
    }
    if(isCommand){await groupCommand(content.trim());return}
    if(chat==='admin'){
      if(!session.value)await newChat()
      if(!same())return
      running.value=true
      await c.request(base()+`/sessions/${session.value}/messages`,post({content}))
    }else{
      const row=await c.request(`/v1/repl/${p}/groups/${chat}/messages`,post({content,previous_session_id:null}))
      if(!same())return
      session.value=row.id
    }
    if(!same())return
    attachments.value=[]
    refresh().catch(e=>{if(same()&&e.name!=='AbortError')error.value=e.message})
    // The send has been accepted. Metadata failures must not restore/re-send this message.
    c.request(`/v1/repl/${p}/chats/${encodeURIComponent(chat)}/history?limit=1`).then(current=>{
      if(!same())return
      current.events?.forEach(add)
      if(chat!=='admin'){session.value=current.active_run||session.value;running.value=!!current.active_run}
    }).catch(e=>{if(same()&&e.name!=='AbortError')error.value='消息已提交，但刷新记录失败：'+e.message})
  }catch(e){if(same()){pendingMessage.value=null;running.value=wasRunning;if(!text.value)text.value=draft}throw e}finally{busy.value=false}
}
async function interrupt(){
  if(!session.value)return
  const path=selected.value==='admin'?base()+`/sessions/${session.value}/interrupt`:`/v1/sessions/${session.value}/interrupt`
  await connection.request(path,post({}))
}
async function decide(item,allow,workspace=false,conversation=false){
  const path=workspace?`/v1/workspace/approvals/${item.id}`:base()+`/approvals/${item.id}`
  await connection.request(path,post({allow,conversation}));await refresh()
}
function enter(event){
 // The `@` picker owns these keys while it is open, so an Agent can be chosen
 // with the keyboard alone. Everything else still belongs to the textarea.
 const action=mentionKey(event.key,mentionChoices.value,event.isComposing)
 if(action==='next'){event.preventDefault();mentionIndex.value=moveHighlight(mentionIndex.value,1,mentionChoices.value.length);return}
 if(action==='previous'){event.preventDefault();mentionIndex.value=moveHighlight(mentionIndex.value,-1,mentionChoices.value.length);return}
 if(action==='accept'){event.preventDefault();const name=highlighted(mentionChoices.value,mentionIndex.value);if(name)insertMention(name);else dismissMention();return}
 if(action==='dismiss'){event.preventDefault();dismissMention();return}
 if(event.key==='Enter'&&!event.shiftKey&&!event.isComposing){event.preventDefault();perform(send)}
}
function dismissMention(){dismissedAt.value=mentionAtCursor.value?mentionAtCursor.value.start:-1}
onMounted(()=>perform(async()=>{try{await connect()}catch(e){settings.value=true;throw e}}))
onBeforeUnmount(close)
</script>
<template>
  <main class="workspace" :class="{'show-list':mobileList}">
    <aside class="sidebar">
      <div class="brand"><span class="brand-name">Crabot</span><el-button type="default" native-type="button" class="icon-button theme-toggle" :aria-label="theme==='dark'?'切换日间模式':'切换夜间模式'" :title="theme==='dark'?'日间模式':'夜间模式'" @click="theme=theme==='dark'?'light':'dark'"><el-icon><Sunny v-if="theme==='dark'"/><Moon v-else/></el-icon></el-button><el-button type="default" native-type="button" class="icon-button" aria-label="Crabot 设置" @click="settings=true"><el-icon><Setting/></el-icon></el-button></div>
      <el-tabs class="workspace-tabs" :model-value="tab" @tab-change="value=>perform(()=>selectTab(value))"><el-tab-pane label="项目" name="projects" :disabled="!connected"/><el-tab-pane label="管理" name="management" :disabled="!connected"/></el-tabs>
      <div v-if="tab==='projects'" class="sidebar-heading">项目 <span class="group-actions"><el-button type="default" native-type="button" class="icon-button" :disabled="!connected||!project" @click="openPanel('new-group')" aria-label="创建项目" title="创建项目"><el-icon><Plus/></el-icon></el-button><el-button type="default" native-type="button" class="icon-button" :disabled="!connected" @click="perform(()=>refresh(true))" aria-label="刷新项目" title="刷新项目"><el-icon><Refresh/></el-icon></el-button></span></div>
      <div v-if="tab==='management'"><el-button type="default" native-type="button" class="agent-choice" :class="{active:!agentsOpen&&!networkOpen&&!toolsOpen&&!skillsOpen&&!allowlistOpen&&!promptsOpen&&!docsOpen}" @click="closePanel">管理</el-button><el-button type="default" native-type="button" class="agent-choice" :class="{active:networkOpen}" @click="openPanel('connections')">组网</el-button><el-button class="agent-choice" :class="{active:allowlistOpen}" @click="openPanel('allowlist')">权限白名单</el-button><el-button class="agent-choice" :class="{active:promptsOpen}" @click="openPanel('prompts')">系统提示词</el-button><div class="sidebar-heading">Agents <span class="group-actions"><el-button class="icon-button" :loading="agentsRefreshing" :disabled="!connected" aria-label="刷新 Agents" title="刷新 Agents" @click="refreshAgents"><el-icon v-if="!agentsRefreshing"><Refresh/></el-icon></el-button><el-button type="default" native-type="button" class="icon-button" aria-label="新增 Agent" @click="openPanel('agents')"><el-icon><Plus/></el-icon></el-button></span></div><el-button type="default" native-type="button" class="agent-choice" v-for="a in managedAgents" :key="a.kind+a.id" :class="{active:agentsOpen&&route.query.agent===a.id}" @click="inspectAgent(a.id)"><el-avatar :size="32" shape="square" :icon="a.kind==='remote'?Connection:User"/><span><b>{{a.name||a.id}}</b><small v-if="a.kind==='remote'">{{a.remote_address||'地址未知'}}</small><small>{{a.is_default?'默认 Agent':a.kind==='virtual'?'虚拟 Agent':a.kind==='remote'?'远端':a.provider}} · {{a.kind==='virtual'?'已配置':a.online?'在线':'已停止'}}</small></span></el-button></div>
      <template v-if="tab==='projects'"><div class="chat-list-row" v-for="g in groups" :key="g.key"><el-button type="default" native-type="button" class="agent-choice" :class="{active:selected===g.key}" @click="perform(()=>choose(g.key))"><el-avatar :size="32" shape="square" :icon="ChatDotRound"/><span><b>{{g.body.name}}</b><small>{{g.body.policy.mode==='chat'?'简单聊天':g.body.policy.mode==='relay'?'接力模式':g.body.policy.mode==='a2a'?'讨论模式':'Leader 模式'}}</small></span></el-button><ChatMenu :group="g" :request="apiRequest" @changed="perform(()=>refresh(true))" @deleted="async id=>{if(selected===id)await choose(null);await perform(()=>refresh(true))}"/></div><p v-if="!groups.length" class="no-history">暂无项目，点击 ＋ 创建。</p></template>
      <div class="capability-nav"><el-button type="default" native-type="button" class="secondary" :disabled="!connected" @click="openPanel('tools')">{{tab==='management'?'管理工具库':'项目工具库'}}</el-button><el-button type="default" native-type="button" class="secondary" :disabled="!connected" @click="openPanel('skills')">{{tab==='management'?'管理 Skill 库':'项目 Skill 库'}}</el-button><el-button v-if="tab==='projects'" class="secondary" :disabled="!connected" @click="openPanel('docs')">知识库</el-button></div>
      <div class="sidebar-spacer"></div>
      <footer class="sidebar-footer"><span class="online-dot" :class="{offline:!connected}"></span>{{connected?'已连接 · 本地数据':'未连接'}}</footer>
    </aside>
    <section v-if="selected&&!agentsOpen&&!networkOpen&&!toolsOpen&&!skillsOpen&&!allowlistOpen&&!promptsOpen&&!docsOpen" class="chat">
      <header class="chat-header"><el-button type="default" native-type="button" class="icon-button mobile-toggle" @click="mobileList=!mobileList" aria-label="聊天列表"><el-icon><Menu/></el-icon></el-button><div><h2>{{title}}</h2><p><span class="online-dot"></span>{{running?'正在协作':'随时准备好'}} <span class="header-separator">/</span> {{selected==='admin'?'用对话管理你的 Agent':selectedGroup?.body.policy.mode==='chat'?'单 Agent 对话':'多 Agent 协作空间'}}</p></div><div class="header-actions"><ProcessSessions :key="project+':'+selected" :project="project" :chat="selected" :request="apiRequest"/><el-button type="default" native-type="button" v-if="selected!=='admin'" class="secondary toolbar-icon" @click="openPanel('group')" aria-label="项目配置" title="项目配置"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" aria-hidden="true"><path d="M4 7h16M4 17h16"/><circle cx="9" cy="7" r="3" fill="currentColor"/><circle cx="16" cy="17" r="3" fill="currentColor"/></svg></el-button><el-button type="default" native-type="button" class="secondary" :disabled="!connected" @click="perform(newChat)" aria-label="刷新聊天"><el-icon><Refresh/></el-icon></el-button></div></header>

      <div v-if="error" role="alert" class="error-banner">{{error}}<el-button type="default" native-type="button" class="icon-button" aria-label="关闭提示" @click="error=''">×</el-button></div>
      <ChatMessages :has-more="hasMore" :load-older="older" @history-error="error=$event" :key="topicId||selected" :messages="messages" :running="running" :progress="progress" :management="selected==='admin'" @suggest="value=>text=value" />
      <div class="composer-area" @dragover.prevent @drop="attachmentPicker?.drop($event)" @paste="attachmentPicker?.paste($event)">


      <div v-if="commandReply" class="command-reply"><el-button type="default" native-type="button" class="icon-button" aria-label="关闭命令结果" @click="commandReply=''">×</el-button><MarkdownText :text="commandReply" /><small>/add-agent PATH [角色] · /remove-agent PATH · /agent PATH role 新角色 · /help</small></div>
        <div class="composer-feedback">
          <ApprovalCard v-for="a in approvals" :key="a.id" :item="a" @decide="allow=>perform(()=>decide(a,allow))"/>
          <ApprovalCard v-for="a in visiblePaths" :key="a.id" :item="a" workspace @conversation="perform(()=>decide(a,true,true,true))" @decide="allow=>perform(()=>decide(a,allow,true))"/>
        </div>
        
<p v-if="mentionHintText" class="composer-note">{{mentionHintText}}</p><div v-if="mentionChoices.length" class="mention-popup" role="listbox" aria-label="选择要对话的 Agent"><button v-for="(name,index) in mentionChoices" :key="name" type="button" role="option" :aria-selected="index===mentionIndex" :class="['mention-option',{active:index===mentionIndex}]" @mousedown.prevent="insertMention(name)" @mouseenter="mentionIndex=index"><code>@{{name}}</code><span>只与该 Agent 对话</span></button><small class="mention-hint">↑↓ 选择 · Enter 确认 · Esc 关闭</small></div><div v-if="suggestions.length" class="command-suggestions" aria-label="群聊命令提示"><el-button type="primary" native-type="button" v-for="item in suggestions" :key="item.name" @click="insertCommand(['/agents','/help'].includes(item.name)?item.name:item.name+' ')"><code>{{item.usage}}</code><span>{{item.description}}</span></el-button></div><el-form class="composer" @submit.prevent="perform(send)" label-position="top"><el-input type="textarea" v-model="text" @keydown="enter" :disabled="!connected||!project" aria-label="消息"  :placeholder="selected==='admin'?'输入管理指令或问题…':groupMembers.length>1?'发送消息；@名字 可只与该 Agent 对话，或 /add-agent、/remove-agent、/agent…':'发送消息'" rows="3"></el-input><div class="composer-toolbar"><div class="composer-left"><PermissionControl v-if="selectedGroup?.body.policy.members.length===1&&selectedGroup.body.policy.members[0].path.length===1&&managedAgents.some(a=>a.kind==='local'&&a.id===selectedGroup.body.policy.members[0].path[0])" compact :project="project" :agent="selectedGroup.body.policy.members[0].path[0]" :request="apiRequest"/><el-button type="default" :disabled="!connected||busy||running" @click="perform(resetContext)" title="不删除历史记录，仅重置后续对话上下文">重置上下文</el-button></div><AttachmentPicker :key="project+':'+selected" ref="attachmentPicker" v-model="attachments" :request="apiRequest" :disabled="!connected||!project||busy||running" @uploading="uploading=$event" @error="error=$event"/><el-button type="default" native-type="button" v-if="running&&!(selected!=='admin'&&text.trim().startsWith('/'))" class="secondary" :disabled="busy" @click="perform(interrupt)">{{busy?'发送中…':'■ 停止生成'}}</el-button><el-button type="primary" native-type="button" @click="perform(send)" :loading="busy" :disabled="!connected||!project||busy||uploading||(!text.trim()&&!attachments.length)" class="send-button">{{running?'发送引导 ↑':'发送 ↑'}}</el-button></div></el-form><p class="composer-note">数据保存在当前 Agent · 重要操作会先征求你的确认</p></div>
    </section>
    <section v-else-if="!agentsOpen&&!networkOpen&&!toolsOpen&&!skillsOpen&&!allowlistOpen&&!promptsOpen&&!docsOpen" class="empty-project"><el-button type="default" native-type="button" class="secondary mobile-toggle" @click="mobileList=true">查看项目与设置</el-button><h2>项目</h2><p>创建一个项目，邀请 Agent 开始协作。</p><el-button type="primary" native-type="button" :disabled="!connected||!project" @click="openPanel('new-group')">＋ 创建项目</el-button></section>
    <NetworkConfiguration v-if="networkOpen" :project="project" :request="apiRequest" @close="closePanel"/>
    <AgentManagement v-if="agentsOpen" :project="project"  :request="apiRequest" :stream="apiStream" :selected-agent="route.query.agent" @select="inspectAgent" @changed="perform(()=>refresh(true))" @close="closePanel"/>
    <div v-if="newGroupOpen" class="panel-mount"><GroupConfiguration :group="emptyGroup" :project="project" :request="apiRequest" creating @close="closePanel" @saved="groupCreated"/></div>
    <div v-if="membersOpen&&selectedGroup" class="panel-mount"><GroupConfiguration :group="selectedGroup" :project="project" :request="apiRequest" @close="closePanel" @saved="updated=>{groups=groups.map(g=>g.key===updated.key?{...updated,namespace_id:project}:g)}" /></div>
    <SystemPrompts v-if="promptsOpen&&connected" :key="connectedAddress" :request="apiRequest"/>
    <DocumentLibrary v-if="docsOpen&&connected" :key="'docs:'+connectedAddress" :request="apiRequest" @switch="openPanel" @close="closePanel"/>
    <section v-if="allowlistOpen" class="allowlist-page"><h2>权限白名单</h2><p>当前 Crabot 实例全局共享</p><CommandAllowlist :key="allowlistVersion" :request="apiRequest" url="/v1/permissions/allowlist" @saved="allowlistVersion++" @close="closePanel"/></section>
    <CapabilityLibrary v-if="skillsOpen||toolsOpen" @switch="openPanel" :kind="toolsOpen?'tool':'skill'" :project="project" :scope="tab==='management'?'management':'business'" :group="tab==='projects'?selected:''" :request="apiRequest" @close="closePanel"/>
    <el-dialog v-model="settings" title="连接与设置" width="min(520px,94vw)" :close-on-click-modal="connected" :close-on-press-escape="connected" :show-close="connected" align-center>
      <section class="settings-panel">
        <p>管理页面仅允许本机访问，无需口令。远端 Agent 请通过组网连接；修改其配置需打开所属节点的页面（或使用 SSH 本地端口转发）。</p>
        <el-form class="connection-form" @submit.prevent="perform(connect)" label-position="top"><label>Agent 地址<el-input v-model="address" aria-label="Agent 地址" required placeholder="http://127.0.0.1:8787" /></label><el-button type="primary" native-type="submit">连接 Agent →</el-button></el-form>
        <p class="error-text" v-if="error">{{error}}</p>
      </section>
    </el-dialog>
  </main>
</template>

<style scoped>.chat-list-row{position:relative;display:flex;align-items:center;min-width:0}.chat-list-row>.agent-choice{flex:1;min-width:0;padding-right:42px}.chat-list-row :deep(.el-dropdown){position:absolute;right:8px;top:50%;transform:translateY(-50%)}.chat-list-row :deep(.chat-menu){padding:6px;margin:0}</style>
