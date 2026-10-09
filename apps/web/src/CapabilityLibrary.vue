<script setup>
import {ref,computed,onMounted,watch} from 'vue'
import ConfigPanel from './ConfigPanel.vue'
import MarkdownText from './MarkdownText.vue'
import ToolTest from './ToolTest.vue'
import {localCapabilityAgents} from './capability-targets.js'
import CapabilityAgents from './CapabilityAgents.vue'
import {parseSkillImport,parseToolImport} from './capability-import.js'
const props=defineProps({request:Function,project:String,group:String,scope:String,kind:String})
const emit=defineEmits(['close','switch'])
function switchKind(kind){if(dirty.value){error.value='请先保存或取消编辑后切换。';return}emit('switch',kind==='doc'?'docs':kind==='tool'?'tools':'skills')}
const contextProject=ref(props.project),contextGroup=ref(props.group||''),projectCatalog=ref([]),agent=ref(props.scope==='management'?'admin':''),data=ref({rows:[],projects:[],agents:[]})
const panel=ref('')
const drawerOpen=computed({get:()=>!!panel.value,set:value=>{if(!value)panel.value=''}})
function closeDrawer(done){if(busy.value)return;if(dirty.value){error.value='请先保存或取消编辑。';return}panel.value='';done()}
function openResource(row,view){select(row);panel.value=view}
const targetReady=ref(false)
const selected=ref(''),query=ref(''),busy=ref(false),error=ref(''),notice=ref(''),draft=ref(null),baseline=ref(''),editing=ref(false),file=ref('SKILL.md'),preview=ref(false),upload=ref(null),folder=ref(null),newFile=ref(''),remove=ref(false)
const layerLabels={global:'全局',project:'项目',agent:'Agent',project_agent:'项目内 Agent'}
const dirty=computed(()=>editing.value&&JSON.stringify(draft.value)!==baseline.value)
const current=computed(()=>data.value.rows.find(r=>r.resource.id===selected.value))
const filtered=computed(()=>data.value.rows.filter(r=>JSON.stringify([name(r.resource),r.resource.definition.description]).toLowerCase().includes(query.value.toLowerCase())))
const agents=computed(()=>[...new Set(localCapabilityAgents(data.value.agents,projectCatalog.value.find(p=>p.key===contextGroup.value)).map(a=>a.client_id))].sort())
const localTarget=computed(()=>props.scope==='management'||data.value.agents.some(a=>a.project_id===contextProject.value&&a.client_id===agent.value))
const canTest=computed(()=>targetReady.value&&localTarget.value&&agent.value&&current.value?.resolution.enabled&&!editing.value)
const endpoint=()=>'/v1/repl/'+contextProject.value+'/capabilities/'+props.scope+'/'+props.kind
const queryString=()=>'?agent='+encodeURIComponent(agent.value)+(contextGroup.value?'&group='+encodeURIComponent(contextGroup.value):'')
function name(resource){return resource.definition.name||resource.definition.id}
let loadVersion=0
async function load(){const version=++loadVersion;targetReady.value=false;const result=await props.request(endpoint()+queryString());if(version!==loadVersion)return;data.value=result;if(!data.value.rows.some(r=>r.resource.id===selected.value))selected.value=data.value.rows[0]?.resource.id||'';targetReady.value=true}
async function perform(action){if(busy.value)return;busy.value=true;error.value='';notice.value='';try{await action()}catch(e){error.value=e.message}finally{busy.value=false}}
watch(()=>props.kind,()=>{panel.value='';editing.value=false;selected.value='';query.value='';file.value='SKILL.md';skillDirectory.value='';testScript.value='';load().catch(e=>{error.value=e.message})})
onMounted(()=>perform(async()=>{const index=await props.request('/v1/repl');projectCatalog.value=index.collaboration_projects||[];await load()}))
async function changeContext(){
 const row=projectCatalog.value.find(p=>p.key===contextGroup.value)
 if(row)contextProject.value=row.namespace_id
 if(!agents.value.includes(agent.value))agent.value=''
 await perform(load)
}
function select(row){if(dirty.value){error.value='请先保存或取消编辑。';return}selected.value=row.resource.id;editing.value=false;remove.value=false;file.value='SKILL.md';error.value=''}
function edit(create=false){
 panel.value='detail'
 if(dirty.value){error.value='请先保存或取消编辑。';return}
 draft.value=create?(props.kind==='skill'?{id:'',description:'',enabled:false,allow_python:false,files:{'SKILL.md':''}}:{name:'',description:'',command:'',enabled:false}):JSON.parse(JSON.stringify(current.value.resource.definition))
 if(create)selected.value=''
 baseline.value=JSON.stringify(draft.value);editing.value=true;file.value='SKILL.md';preview.value=false;remove.value=false
}
async function save(){
 await perform(async()=>{
 const saved=await props.request(endpoint(),{method:'PUT',headers:{'content-type':'application/json'},body:JSON.stringify({id:current.value?.resource.id,expected_version:current.value?.resource.version||0,definition:draft.value})})
 selected.value=saved.id;editing.value=false;await load();notice.value='定义已统一保存；所有绑定位置将在下一轮使用新版本。'
 })
}
async function deleteResource(){
 await perform(async()=>{
 await props.request(endpoint(),{method:'PUT',headers:{'content-type':'application/json'},body:JSON.stringify({id:current.value.resource.id,expected_version:current.value.resource.version,deleted:true})})
 remove.value=false;await load();notice.value='已移除定义；各个项目将不再发现这项能力。'
 })
}
async function importFiles(event){
 const files=[...event.target.files];event.target.value=''
 if(!files.length)return
 await perform(async()=>{
 if(dirty.value)throw Error('请先保存或取消编辑。')
 if(props.kind==='skill'){
 const definition=await parseSkillImport(files);selected.value='';draft.value=definition;baseline.value='';editing.value=true;file.value='SKILL.md';preview.value=false
 }else{
 if(files[0].size>600000)throw Error('文件过大。')
 const definitions=parseToolImport(await files[0].text())
 if(definitions.length!==1)throw Error('请一次导入一个工具定义，检查后保存。')
 selected.value='';draft.value=definitions[0];baseline.value='';editing.value=true
 }
 panel.value='detail';notice.value='导入为停用草稿；请检查内容后保存。不会自动执行或授予权限。'
 })
}
function addFile(){
 const path=newFile.value.trim()
 if(!path||path.length>200||!path.split('/').every(p=>p&&!p.startsWith('.')&&/^[a-zA-Z0-9_.-]+$/.test(p))||Object.hasOwn(draft.value.files,path)){error.value='请输入安全且不重复的相对路径。';return}
 if(Object.keys(draft.value.files).length>=32){error.value='最多 32 个文件。';return}
 draft.value.files[path]='';file.value=path;newFile.value='';preview.value=false
}
const content=computed(()=>editing.value?draft.value:current.value?.resource.definition)
const skillDirectory=ref(''),testScript=ref('')
watch([selected,contextGroup,agent,()=>current.value?.resource.version],()=>{skillDirectory.value='';testScript.value=''})
const quote=value=>"'"+String(value).replaceAll("'","'\\''")+"'"
const scriptCommand=computed(()=>{const file=testScript.value;const interpreter=file.endsWith('.py')?'python3':file.endsWith('.mjs')||file.endsWith('.js')?'node':'sh';return interpreter+' '+quote(skillDirectory.value+'/'+file)})
const scripts=computed(()=>Object.keys(content.value?.files||{}).filter(f=>/\.(py|mjs|js|sh)$/.test(f)))
watch(scripts,()=>{if(!scripts.value.includes(testScript.value))testScript.value=scripts.value[0]||''})
</script>
<template>
<ConfigPanel inline :title="(scope==='management'?'管理':'项目')+'能力库'" eyebrow="SHARED CAPABILITIES" description="统一维护工具与 Skill，在 Agent 表格中直接启用或停用。" :busy="busy" :dirty="dirty" @close="$emit('close')">
 <el-tabs class="capability-tabs" :model-value="kind" @tab-change="switchKind"><el-tab-pane label="工具" name="tool" :disabled="busy"/><el-tab-pane label="Skills" name="skill" :disabled="busy"/><el-tab-pane v-if="scope!=='management'" label="知识库" name="doc" :disabled="busy"/></el-tabs>
 <div v-if="error" class="config-alert danger" role="alert">{{error}}</div><div v-if="notice" class="config-alert success" role="status">{{notice}}</div>
 <input ref="upload" type="file" :multiple="kind==='skill'" :accept="kind==='tool'?'.json':undefined" hidden @change="importFiles"><input ref="folder" type="file" webkitdirectory multiple hidden @change="importFiles">
 <div class="library-body">
 <div class="library-toolbar"><el-input v-model="query" clearable :placeholder="kind==='tool'?'搜索工具名称或描述':'搜索 Skill 名称或描述'" aria-label="搜索能力"/><div class="library-actions">
 <template v-if="kind==='skill'||scope==='business'"><el-button :disabled="busy||dirty" @click="edit(true)">{{kind==='tool'?'注册工具':'新建 Skill'}}</el-button><el-button :disabled="busy||dirty" @click="upload.click()">上传定义</el-button><el-button v-if="kind==='skill'" :disabled="busy||dirty" @click="folder.click()">上传文件夹</el-button></template>
 <el-button :disabled="busy||dirty" @click="perform(load)">刷新</el-button></div></div>
 <el-table :data="filtered" row-key="resource.id" empty-text="暂无条目，新建或上传一个定义开始使用" class="library-table">
 <el-table-column :label="kind==='tool'?'工具':'Skill'" width="180" show-overflow-tooltip><template #default="{row}"><el-button link type="primary" :disabled="busy" @click="openResource(row,'detail')"><span class="resource-name">{{name(row.resource)}}</span></el-button></template></el-table-column>
 <el-table-column label="说明" min-width="280"><template #default="{row}"><span class="resource-description">{{row.resource.definition.description}}</span></template></el-table-column>
 <el-table-column label="来源" width="80"><template #default="{row}"><el-tag size="small" :type="row.resource.readonly?'info':'success'">{{row.resource.readonly?'内置':row.resource.origin?'旧配置':'自定义'}}</el-tag></template></el-table-column>
 <el-table-column label="测试" width="90"><template #default="{row}"><el-button link type="primary" :disabled="busy||dirty" @click="openResource(row,'test')">测试运行</el-button></template></el-table-column>
 <el-table-column label="生效规则" width="100" fixed="right"><template #default="{row}"><el-button link type="primary" :disabled="busy" @click="openResource(row,'rules')">配置范围</el-button></template></el-table-column>
 </el-table><p class="library-note">统一维护定义，按项目与 Agent 控制启用。管理能力与项目能力隔离。</p>
 </div>
 <el-drawer v-model="drawerOpen" :before-close="closeDrawer" direction="rtl" size="min(760px,100vw)" :title="panel==='test'?'测试运行':panel==='rules'?'生效规则':'能力详情'" append-to-body destroy-on-close class="capability-drawer">
 <div v-if="error" class="config-alert danger" role="alert">{{error}}</div><div v-if="notice" class="config-alert success" role="status">{{notice}}</div>
 <label v-if="panel==='test'" class="field-stack">{{kind==='tool'?'选择工具':'选择 Skill'}}<el-select v-model="selected" filterable :disabled="busy" aria-label="选择测试能力" @change="()=>{file='SKILL.md';skillDirectory='';testScript=''}"><el-option v-for="row in data.rows" :key="row.resource.id" :value="row.resource.id" :label="name(row.resource)"/></el-select></label>
  <section class="config-detail">
   <el-form v-if="editing" @submit.prevent="save" label-position="top"><div class="section-heading"><h3>{{current?'编辑统一定义':'新增统一定义'}}</h3><p>内容修改会影响所有引用位置；启用范围在保存后单独设置。</p></div>
    <fieldset class="config-fields" :disabled="busy"><div class="form-grid"><label>{{kind==='skill'?'Skill ID':'工具名'}}<el-input v-if="kind==='skill'" v-model="draft.id" :readonly="!!current" required pattern="[a-zA-Z0-9_-]{1,64}" /><el-input v-else v-model="draft.name" :readonly="!!current" required pattern="[a-zA-Z0-9_]{1,64}" /></label><label>用途描述<el-input v-model="draft.description" required maxlength="2048" /></label></div>
    <label class="field-stack checkbox-field"><span><el-checkbox v-model="draft.enabled" /> {{current?.resource.origin?'旧绑定的默认状态（保留原范围）':'全局默认启用'}} · 四层显式设置可以覆盖</span></label>
    <label v-if="kind==='tool'" class="field-stack">固定 Shell 命令<el-input type="textarea" class="code-editor" v-model="draft.command" rows="7" required maxlength="8192"></el-input></label>
    <template v-else><div class="file-workspace"><aside class="file-tree"><el-button type="primary" native-type="button" v-for="(_,path) in draft.files" :key="path" :class="{active:file===path}" @click="file=path">{{path}}</el-button></aside><div class="file-editor"><div class="file-editor-bar"><code>{{file}}</code><el-button type="default" native-type="button" class="quiet-button" @click="preview=!preview">{{preview?'编辑源码':'预览'}}</el-button></div><MarkdownText v-if="preview&&file.endsWith('.md')" class="skill-preview" :text="draft.files[file]"/><el-input type="textarea" v-else v-model="draft.files[file]" class="code-editor" aria-label="Skill 文件内容" rows="12" spellcheck="false"></el-input></div></div><div class="inline-form"><label>新增资源文件<el-input v-model="newFile" placeholder="references/guide.md" @keydown.enter.prevent="addFile" /></label><el-button type="default" native-type="button" class="secondary" @click="addFile">添加文件</el-button></div></template>
    <div class="security-note">启用不是授权。命令仍需逐次确认，Python 仍受宿主与 Skill 授权约束。上传不会自动执行。</div>
    <div class="editor-actions"><el-button type="default" native-type="button" class="secondary" @click="editing=false">取消编辑</el-button><el-button type="primary" native-type="submit" :disabled="busy">保存统一定义</el-button></div></fieldset>
   </el-form>
   <template v-else-if="current">
    <div class="detail-hero"><el-button v-if="panel==='detail'" class="detail-test-button" type="primary" :disabled="busy" @click="panel='test'">测试运行</el-button><span class="detail-symbol">{{kind==='tool'?'⌘':'▤'}}</span><div><span class="eyebrow">{{current.resource.readonly?'BUILT-IN':'SHARED DEFINITION'}}</span><h3>{{name(current.resource)}}</h3><span v-if="panel!=='rules'" class="status-pill" :class="{muted:!current.resolution.enabled}">所选目标 · {{current.resolution.enabled?'启用':'停用'}}</span><span v-if="panel!=='rules'" class="muted">来源：{{layerLabels[current.resolution.source]||'定义默认值 / 旧绑定'}}</span></div></div>
    <p class="detail-description">{{content.description}}</p>
    <CapabilityAgents v-if="panel==='rules'" :key="selected" :request="request" :project="project" :scope="scope" :kind="kind" :resource="selected" :projects="projectCatalog" @changed="perform(load)"/>
    <div v-if="panel==='test'" class="form-card"><div class="section-heading"><h4>{{panel==='test'?'测试目标':'生效目标与启用规则'}}</h4><p>选择执行测试的项目和 Agent，不会修改启用范围。</p></div> <div class="capability-targets">
  <label v-if="scope==='business'" class="agent-selector">目标项目<el-select v-model="contextGroup" :disabled="busy||editing" @change="changeContext"><el-option value="" :label="&quot;全局（未指定项目）&quot;" /><el-option v-for="p in projectCatalog" :key="p.key" :value="p.key" :label="(p.body.name)" /></el-select></label>
  <label v-if="scope==='business'" class="agent-selector">Agent<el-select v-model="agent" :disabled="busy||editing" @change="perform(load)"><el-option value="" :label="&quot;未指定（仅全局与项目）&quot;" /><el-option v-for="id in agents" :key="id" :label="(id)" :value="(id)" /></el-select></label>
  <span v-else class="scope-label">管理 Agent</span>
  <el-button type="default" native-type="button" class="secondary" :disabled="busy||dirty" @click="perform(load)">刷新</el-button>
 </div>

     <p class="binding-note" v-if="!agent">选择 Agent 后可配置 Agent 和项目内 Agent 两层。</p><p class="binding-note" v-else-if="scope==='business'">Agent 层按本节点 client_id「{{agent}}」跨项目生效；项目内 Agent 层只影响当前项目。</p>
     <p v-if="current.resource.origin" class="binding-note">旧定义默认只在原项目{{current.resource.origin.agent?' / '+current.resource.origin.agent:''}}启用。内容统一维护，不自动扩大范围。同名定义可保留，但不要在同一执行范围同时启用。</p>
    </div>
    <div v-if="panel==='detail'&&kind==='tool'" class="form-card"><h4>{{current.resource.readonly?'参数 Schema':'执行命令'}}</h4><pre class="code-preview">{{current.resource.readonly?JSON.stringify(content.parameters,null,2):content.command}}</pre></div>
    <div v-else-if="panel==='detail'" class="file-workspace"><aside class="file-tree"><el-button type="primary" native-type="button" v-for="(_,path) in content.files" :key="path" :class="{active:file===path}" @click="file=path">{{path}}</el-button></aside><div class="file-editor"><div class="file-editor-bar"><code>{{file}}</code></div><MarkdownText v-if="file.endsWith('.md')" class="skill-preview" :text="content.files[file]||''"/><pre v-else class="code-preview">{{content.files[file]}}</pre></div></div>
    <div v-if="panel==='detail'&&!current.resource.readonly" class="editor-actions"><el-button type="default" native-type="button" class="quiet-button destructive" :disabled="busy" @click="remove=true">删除统一定义</el-button><el-button type="default" native-type="button" class="secondary" :disabled="busy" @click="edit()">编辑定义</el-button></div>
    <div v-if="remove" class="config-alert warning">删除会影响所有项目中对此定义的引用。<el-button type="primary" native-type="button" :disabled="busy" @click="deleteResource">确认删除</el-button><el-button type="default" native-type="button" class="secondary" @click="remove=false">取消</el-button></div>
    <div v-if="panel==='test'" class="skill-test-options"><div class="section-heading"><h4>在当前项目与 Agent 中试运行</h4><p>使用这里显示的最终启用状态；不绕过执行权限。</p></div>
     <p v-if="kind==='skill'" class="muted">先加载 Skill。若包含脚本，加载成功后可通过 shell 测试执行，仍需命令授权。</p>
     <p v-if="!localTarget||!agent" class="muted">请选择当前节点中可运行的本地 Agent 后测试。</p>
     <ToolTest v-if="agent" :key="selected+contextProject+contextGroup+agent+current.resolution.enabled" :request="request" :project="contextProject" :group="contextGroup" :scope="scope" :agent="agent" :name="kind==='tool'?name(current.resource):'find'" :schema="kind==='tool'?content.parameters:undefined" :initial-arguments="kind==='skill'?{target:'skill',id:content.id}:undefined" :disabled="busy||!canTest" @completed="output=>{skillDirectory=output?.skills?.[0]?.directory||''}"/>
     <template v-if="kind==='skill'&&skillDirectory&&scripts.length&&scope==='business'">
      <label>执行脚本<el-select v-model="testScript"><el-option v-for="path in scripts" :key="path" :label="path" :value="path" /></el-select></label>
      <ToolTest v-if="testScript" :key="skillDirectory+testScript" :request="request" :project="contextProject" :group="contextGroup" :scope="scope" :agent="agent" name="shell" :initial-arguments="{command:scriptCommand}" :disabled="busy||!canTest"/>
     </template>
    </div>
   </template>
   <div v-else class="config-empty"><span>{{kind==='tool'?'⌘':'▤'}}</span><h3>统一能力库</h3><p>创建或导入一次定义，再按项目与 Agent 绑定使用。</p></div>
  </section>
 </el-drawer>
 <footer class="config-footer"><span>全节点共享定义 · 配置持久化至当前实例</span><span>{{dirty?'有未保存修改':'下一轮执行生效'}}</span></footer>
</ConfigPanel>
</template>

<style scoped>
.library-body{flex:1;min-height:0;overflow:auto;padding:22px 26px}.library-toolbar{display:flex;gap:16px;justify-content:space-between;align-items:center;margin-bottom:22px;flex-wrap:wrap}.library-toolbar>.el-input{width:250px}.library-actions{display:flex;gap:8px;flex-wrap:wrap;margin-left:auto}.library-note{margin-top:18px;color:var(--muted);font-size:12px}.resource-name{display:block;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;max-width:154px}.resource-description{font-size:12px;line-height:1.8}.detail-hero{position:relative;padding-right:115px}.detail-test-button{position:absolute;right:0;top:0}.capability-targets{display:flex;align-items:flex-end;flex-wrap:wrap;gap:14px;padding:16px 0 20px;border-bottom:1px solid var(--line);margin-bottom:18px}
.capability-targets label{flex:1;min-width:170px;display:grid;gap:8px;font-size:12px;color:var(--muted)}
.capability-targets select{width:100%;min-width:0}.capability-targets button{flex:none}
</style>
