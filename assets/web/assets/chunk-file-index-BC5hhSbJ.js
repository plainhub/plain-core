import{n as e}from"./chunk-gql-client-BEcfvxWL.js";var t=`query {
  fileIndexVolumes { id path isAvailable indexMode includedDirs excludedDirs }
  fileIndexDefaults { excludedDirs excludedNames excludedPackages excludedUserLibraries }
}`,n=`mutation($id: ID!, $indexMode: IndexMode!, $includedDirs: [String!]!, $excludedDirs: [String!]!) {
  setFileIndexVolume(id: $id, indexMode: $indexMode, includedDirs: $includedDirs, excludedDirs: $excludedDirs)
}`;async function r(t,r=e()){let{id:i,indexMode:a,includedDirs:o,excludedDirs:s}=t,c=await r(n,{id:i,indexMode:a,includedDirs:o,excludedDirs:s});if(c.errors?.length)throw Error(c.errors[0].message);if(!c.data.setFileIndexVolume)throw Error(`Index configuration write refused`)}var i=`mutation($id: ID!) {
  rebuildFileIndexVolume(id: $id)
}`;function a(e){return e.isAvailable&&(e.indexMode===`BLOCKLIST`||e.indexMode===`ALLOWLIST`&&e.includedDirs.length>0)}export{r as i,t as n,i as r,a as t};