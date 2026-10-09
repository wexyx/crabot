<script setup>
import RuntimeConfigurationFields from './RuntimeConfigurationFields.vue'
import {ElMessage} from 'element-plus/es/components/message/index'
import 'element-plus/es/components/message/style/css'
const emit=defineEmits(['saved'])
import {ref,onMounted} from 'vue'
const opened=ref(['configuration'])
const props=defineProps({request:{type:Function,required:true},expanded:Boolean})
const values=ref({}),loaded=ref(false),key=ref(''),hasKey=ref(false),clearKey=ref(false),error=ref(''),busy=ref(false)
async function load(){
  try {
    error.value='';const data=await props.request('/v1/admin-agent/configuration')
    values.value={ADMIN_AGENT_PROVIDER:'crabot',MODEL_PROVIDER:'openai',MODEL_API:'',MODEL_BASE_URL:'',MODEL_NAME:'',MODEL_SYSTEM_PROMPT:'',CODEX_BIN:'',CLAUDE_BIN:'',OPENCODE_BIN:'',OPENCODE_MODEL:'',OPENCODE_AGENT:'',OPENCODE_AUTO_APPROVE:'',OPENCODE_THINKING:'',OPENCODE_STANDALONE:'',...data.configuration.values}
    hasKey.value=data.configuration.has_api_key;key.value='';clearKey.value=false;loaded.value=true
  }catch(e){error.value=e.message}
}
async function save(){
  if(busy.value)return
  busy.value=true;error.value=''
  try{
    const body={...values.value}
    if(clearKey.value)body.MODEL_API_KEY='';else if(key.value)body.MODEL_API_KEY=key.value
    await props.request('/v1/admin-agent/configuration',{method:'PUT',headers:{'content-type':'application/json'},body:JSON.stringify(body)})
    ElMessage.success({message:'配置已保存并生效。',duration:2500,grouping:true})
    key.value='';await load();emit('saved')
  }catch(e){error.value=e.message}finally{busy.value=false}
}
onMounted(()=>{if(props.expanded)load()})
</script>
<template>
  <el-collapse v-model="opened" class="runtime-configuration" @change="value=>{if(value.length&&!loaded)load()}"><el-collapse-item name="configuration" title="默认 Agent 配置">
    <el-form v-if="loaded" @submit.prevent="save" label-position="top">
      <RuntimeConfigurationFields v-model:provider="values.ADMIN_AGENT_PROVIDER" :values="values" v-model:secret="key" v-model:clear-secret="clearKey" :has-key="hasKey" :request="props.request"/>
      <small class="configuration-note">无运行中任务时可切换；会话历史保留。密钥本地明文保存（0600）；CLI 须预先安装并配置认证。配置会持久保存；显式环境变量优先于已保存配置，启动配置文件仅提供默认值。</small>
      <div class="configuration-actions"><span>配置仅存储在当前 Crabot</span><el-button type="primary" native-type="submit" :disabled="busy">{{busy?'保存中…':'保存并切换'}}</el-button></div>
    </el-form>
    <p v-if="error" role="alert">{{error}}</p>
  </el-collapse-item></el-collapse>
</template>
<style scoped>
.runtime-configuration{border:1px solid var(--line);border-radius:16px;background:var(--panel);overflow:hidden}
summary{padding:20px 24px;font-size:15px;font-weight:600;cursor:pointer;border-bottom:1px solid var(--line)}
form{padding:24px;display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:22px 24px}
label{display:grid;gap:9px;color:var(--muted);font-size:12px}input,select{width:100%;min-width:0;box-sizing:border-box}
.clear-secret{display:flex;align-items:center;align-self:end;min-height:38px}.clear-secret input{width:auto}
.configuration-note{grid-column:1/-1;padding:14px 16px;background:var(--raised);border-radius:10px;line-height:1.8;color:var(--muted)}
.configuration-actions{grid-column:1/-1;display:flex;align-items:center;justify-content:space-between;gap:16px;border-top:1px solid var(--line);padding-top:18px}
.configuration-actions span{font-size:12px;color:var(--muted)}p{margin:16px 24px}p[role=alert]{color:#e87575}
@media(max-width:650px){form{grid-template-columns:1fr;padding:18px}.configuration-actions{flex-wrap:wrap}}
</style>
