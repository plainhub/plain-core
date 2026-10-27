import{a as e,t}from"./chunk-gql-client-h_Eo2bcz.js";import{yn as n}from"./chunk-_plugin-vue_export-helper-eUnD5Cnv.js";import{t as r}from"./chunk-eventbus-Ca-q0d_H.js";import{At as i,Ct as a,Dt as o,Et as s,Mt as c,Nt as l,Pt as u,Tt as d,jt as f,kt as p,wt as m}from"./chunk-query-BSEHJWxg.js";function ee(i,a=!0){let o=n(!1),s=[],c=[];async function l(n){o.value=!0;try{let o=await e(i.document,n,{dedupe:!1});if(o.errors?.length){let e=o.errors[0].message;a&&r.emit(`toast`,e);let n=new t(e);for(let e of c)e(n);return}for(let e of s)e(o);return o}catch(e){if(e instanceof t&&e.cancelled)return;let n=e instanceof t?e.message:`network_error`;a&&r.emit(`toast`,n);for(let t of c)t(e);return}finally{o.value=!1}}function u(e){return s.push(e),{off:()=>{let t=s.indexOf(e);t>=0&&s.splice(t,1)}}}function d(e){return c.push(e),{off:()=>{let t=c.indexOf(e);t>=0&&c.splice(t,1)}}}return{mutate:l,loading:o,onDone:u,onError:d}}async function te(e,t){return await e(t)!=null}var h=`
  mutation {
    clearAppLogs
  }
`,g=`
  mutation setPref($key: String!, $value: JSON!) {
    setPref(key: $key, value: $value)
  }
`,_=`
  mutation updateDeviceName($name: String!) {
    updateDeviceName(name: $name)
  }
`,v=`
  mutation forwardChatItems($id: ID!, $targets: [String!]!) {
    forwardChatItems(id: $id, targets: $targets) {
      ...ChatItemFragment
    }
  }
  ${o}
`,y=`
  mutation sendChatItem($target: String!, $content: String!) {
    sendChatItem(target: $target, content: $content) {
      ...ChatItemFragment
    }
  }
  ${o}
`,b=`
  mutation deleteChatItem($id: ID!) {
    deleteChatItem(id: $id)
  }
`,x=`
  mutation deleteChatItems($query: String!) {
    deleteChatItems(query: $query) {
      affectedCount
    }
  }
`,S=`
  mutation retryChatItem($id: ID!) {
    retryChatItem(id: $id) {
      ...ChatItemFragment
    }
  }
  ${o}
`,C=`
  mutation createChatChannel($name: String!) {
    createChatChannel(name: $name) {
      ...ChatChannelFragment
    }
  }
  ${s}
`,w=`
  mutation updateChatChannel($id: ID!, $name: String!) {
    updateChatChannel(id: $id, name: $name) {
      ...ChatChannelFragment
    }
  }
  ${s}
`,T=`
  mutation deleteChatChannel($id: ID!) {
    deleteChatChannel(id: $id)
  }
`,E=`
  mutation deletePeer($id: ID!) {
    deletePeer(id: $id)
  }
`,D=`
  mutation unpairPeer($id: ID!) {
    unpairPeer(id: $id)
  }
`,O=`
  mutation leaveChatChannel($id: ID!) {
    leaveChatChannel(id: $id)
  }
`,k=`
  mutation addChatChannelMember($id: ID!, $peerId: ID!) {
    addChatChannelMember(id: $id, peerId: $peerId) {
      ...ChatChannelFragment
    }
  }
  ${s}
`,A=`
  mutation removeChatChannelMember($id: ID!, $peerId: ID!) {
    removeChatChannelMember(id: $id, peerId: $peerId) {
      ...ChatChannelFragment
    }
  }
  ${s}
`,j=`
  mutation acceptChatChannelInvite($id: ID!) {
    acceptChatChannelInvite(id: $id)
  }
`,M=`
  mutation declineChatChannelInvite($id: ID!) {
    declineChatChannelInvite(id: $id)
  }
`,N=`
  mutation createDir($path: String!) {
    createDir(path: $path) {
      ...FileFragment
    }
  }
  ${c}
`,P=`
  mutation writeTextFile($path: String!, $content: String!, $overwrite: Boolean!) {
    writeTextFile(path: $path, content: $content, overwrite: $overwrite) {
      ...FileFragment
    }
  }
  ${c}
`,F=`
  mutation renameFile($path: String!, $name: String!) {
    renameFile(path: $path, name: $name)
  }
`,I=`
  mutation copyFile($src: String!, $dst: String!, $overwrite: Boolean!) {
    copyFile(src: $src, dst: $dst, overwrite: $overwrite)
  }
`,L=`
  mutation moveFile($src: String!, $dst: String!, $overwrite: Boolean!) {
    moveFile(src: $src, dst: $dst, overwrite: $overwrite)
  }
`,R=`
  mutation recordMediaPlay($mediaId: ID!, $type: MediaDataType!, $positionMs: Long!) {
    recordMediaPlay(mediaId: $mediaId, type: $type, positionMs: $positionMs)
  }
`,z=`
  mutation saveMediaPlayProgress($mediaId: ID!, $type: MediaDataType!, $positionMs: Long!) {
    saveMediaPlayProgress(mediaId: $mediaId, type: $type, positionMs: $positionMs)
  }
`,B=`
  mutation playAudio($mediaId: ID!) {
    playAudio(mediaId: $mediaId) {
      ...AudioItemFragment
    }
  }
  ${a}
`,V=`
  mutation enqueueAndPlayAudio($mediaId: ID!, $query: String!) {
    addAudioToQueue(query: $query)
    playAudio(mediaId: $mediaId) {
      ...AudioItemFragment
    }
  }
  ${a}
`,H=`
  mutation updateAudioPlayMode($mode: MediaPlayMode!) {
    updateAudioPlayMode(mode: $mode)
  }
`,U=`
  mutation removeAudioFromQueue($mediaId: ID!) {
    removeAudioFromQueue(mediaId: $mediaId)
  }
`,W=`
  mutation addAudioToQueue($query: String!) {
    addAudioToQueue(query: $query)
  }
`,G=`
  mutation clearAudioQueue {
    clearAudioQueue
  }
`,K=`
  mutation reorderAudioQueue($mediaIds: [ID!]!) {
    reorderAudioQueue(mediaIds: $mediaIds)
  }
`,q=`
  mutation deleteMediaItems($type: MediaDataType!, $query: String!) {
    deleteMediaItems(type: $type, query: $query) {
      affectedCount
    }
  }
`,J=`
  mutation trashMediaItems($type: MediaDataType!, $query: String!) {
    trashMediaItems(type: $type, query: $query) {
      affectedCount
    }
  }
`,Y=`
  mutation restoreMediaItems($type: MediaDataType!, $query: String!) {
    restoreMediaItems(type: $type, query: $query) {
      affectedCount
    }
  }
`,X=`
  mutation moveMediaItems($type: MediaDataType!, $query: String!, $destDir: String!) {
    moveMediaItems(type: $type, query: $query, destDir: $destDir) {
      affectedCount
    }
  }
`,Z=`
  mutation trashSms($query: String!) {
    trashSms(query: $query) {
      affectedCount
    }
  }
`,Q=`
  mutation restoreSms($query: String!) {
    restoreSms(query: $query) {
      affectedCount
    }
  }
`,ne=`
  mutation deleteSms($query: String!) {
    deleteSms(query: $query) {
      affectedCount
    }
  }
`,re=`
  mutation removeFromTags($type: DataType!, $tagIds: [ID!]!, $query: String!) {
    removeFromTags(type: $type, tagIds: $tagIds, query: $query)
  }
`,ie=`
  mutation addToTags($type: DataType!, $tagIds: [ID!]!, $query: String!) {
    addToTags(type: $type, tagIds: $tagIds, query: $query)
  }
`,ae=`
  mutation updateTagRelations($type: DataType!, $keys: [String!]!, $addTagIds: [ID!]!, $removeTagIds: [ID!]!) {
    updateTagRelations(type: $type, keys: $keys, addTagIds: $addTagIds, removeTagIds: $removeTagIds)
  }
`,oe=`
  mutation createTag($type: DataType!, $name: String!) {
    createTag(type: $type, name: $name) {
      ...TagFragment
    }
  }
  ${u}
`,se=`
  mutation updateTag($id: ID!, $name: String!) {
    updateTag(id: $id, name: $name) {
      ...TagFragment
    }
  }
  ${u}
`,ce=`
  mutation deleteTag($id: ID!) {
    deleteTag(id: $id)
  }
`,le=`
  mutation addFavoriteFolder($rootPath: String!, $fullPath: String!) {
    addFavoriteFolder(rootPath: $rootPath, fullPath: $fullPath) {
      rootPath
      fullPath
    }
  }
`,ue=`
  mutation removeFavoriteFolder($fullPath: String!) {
    removeFavoriteFolder(fullPath: $fullPath) {
      rootPath
      fullPath
      alias
    }
  }
`,de=`
  mutation setFavoriteFolderAlias($fullPath: String!, $alias: String!) {
    setFavoriteFolderAlias(fullPath: $fullPath, alias: $alias) {
      rootPath
      fullPath
      alias
    }
  }
`,fe=`
  mutation createNote($input: NoteInput!) {
    createNote(input: $input) {
      ...NoteFragment
    }
  }
  ${l}
`,pe=`
  mutation updateNote($id: ID!, $input: NoteInput!) {
    updateNote(id: $id, input: $input) {
      ...NoteFragment
    }
  }
  ${l}
`,me=`
  mutation deleteNotes($query: String!) {
    deleteNotes(query: $query) {
      affectedCount
    }
  }
`,he=`
  mutation trashNotes($query: String!) {
    trashNotes(query: $query) {
      affectedCount
    }
  }
`,ge=`
  mutation restoreNotes($query: String!) {
    restoreNotes(query: $query) {
      affectedCount
    }
  }
`,_e=`
  mutation deleteFeedEntries($query: String!) {
    deleteFeedEntries(query: $query) {
      affectedCount
    }
  }
`,ve=`
  mutation deleteCalls($query: String!) {
    deleteCalls(query: $query) {
      affectedCount
    }
  }
`,ye=`
  mutation deleteContacts($query: String!) {
    deleteContacts(query: $query) {
      affectedCount
    }
  }
`,be=`
  mutation createFeed($url: String!, $fetchContent: Boolean!) {
    createFeed(url: $url, fetchContent: $fetchContent) {
      ...FeedFragment
    }
  }
  ${f}
`,xe=`
  mutation importFeeds($content: String!) {
    importFeeds(content: $content)
  }
`,Se=`
  mutation exportFeeds {
    exportFeeds
  }
`,Ce=`
  mutation exportNotes($query: String!) {
    exportNotes(query: $query)
  }
`,we=`
  mutation relaunchApp {
    relaunchApp
  }
`,Te=`
  mutation openAccessibilitySettings {
    openAccessibilitySettings
  }
`,Ee=`
  mutation openWebSettings($feature: WebSettingsFeature) {
    openWebSettings(feature: $feature)
  }
`,De=`
  mutation deleteFeed($id: ID!) {
    deleteFeed(id: $id)
  }
`,Oe=`
  mutation updateFeed($id: ID!, $name: String!, $fetchContent: Boolean!) {
    updateFeed(id: $id, name: $name, fetchContent: $fetchContent) {
      ...FeedFragment
    }
  }
  ${f}
`,ke=`
  mutation syncFeeds($id: ID) {
    syncFeeds(id: $id)
  }
`,Ae=`
  mutation syncFeedEntryContent($id: ID!) {
    syncFeedEntryContent(id: $id) {
      ...FeedEntryFragment
      feed {
        ...FeedFragment
      }
    }
  }
  ${f}
  ${i}
`,je=`
  mutation call($number: String!, $showDialer: Boolean!) {
    call(number: $number, showDialer: $showDialer)
  }
`,Me=`
  mutation setClipboard($text: String!) {
    setClipboard(text: $text)
  }
`,Ne=`
  mutation sendSms($number: String!, $body: String!, $subscriptionId: Int!) {
    sendSms(number: $number, body: $body, subscriptionId: $subscriptionId)
  }
`,Pe=`
  mutation sendSms($number: String!, $body: String!, $subscriptionId: Int!, $requestId: String!) {
    sendSms(number: $number, body: $body, subscriptionId: $subscriptionId, requestId: $requestId)
  }
`,Fe=`
  mutation archiveSmsConversation($id: String!) {
    archiveSmsConversation(id: $id)
  }
`,Ie=`
  mutation unarchiveSmsConversation($id: String!) {
    unarchiveSmsConversation(id: $id)
  }
`,Le=`
  mutation sendMms($number: String!, $body: String!, $attachmentPaths: [String!]!, $threadId: ID!) {
    sendMms(number: $number, body: $body, attachmentPaths: $attachmentPaths, threadId: $threadId)
  }
`,Re=`
  mutation uninstallPackages($id: ID!) {
    uninstallPackages(ids: [$id])
  }
`,ze=`
  mutation installPackage($path: String!) {
    installPackage(path: $path) {
      id
      updatedAt
      isNew
    }
  }
`,Be=`
  mutation startScreenMirror($audio: Boolean!) {
    startScreenMirror(audio: $audio)
  }
`,Ve=`
  mutation requestScreenMirrorAudio {
    requestScreenMirrorAudio
  }
`,He=`
  mutation stopScreenMirror {
    stopScreenMirror
  }
`,Ue=`
  mutation setTempValue($key: String!, $value: String!) {
    setTempValue(key: $key, value: $value) {
      key
      value
    }
  }
`,We=`
  mutation deleteNotifications($ids: [ID!]!) {
    deleteNotifications(ids: $ids) {
      affectedCount
    }
  }
`,Ge=`
  mutation replyNotification($id: ID!, $actionIndex: Int!, $text: String!) {
    replyNotification(id: $id, actionIndex: $actionIndex, text: $text)
  }
`,Ke=`
  mutation deleteClipboardItems($query: String!) {
    deleteClipboardItems(query: $query) {
      affectedCount
    }
  }
`,qe=`
  mutation updateScreenMirrorQuality($mode: ScreenMirrorMode!) {
    updateScreenMirrorQuality(mode: $mode)
  }
`,Je=`
  mutation saveFeedEntriesToNotes($query: String!) {
    saveFeedEntriesToNotes(query: $query)
  }
`,Ye=`
  mutation mergeChunks($fileId: String!, $totalChunks: Int!, $path: String!, $replace: Boolean!, $totalSize: Long!) {
    mergeChunks(fileId: $fileId, totalChunks: $totalChunks, path: $path, replace: $replace, totalSize: $totalSize) {
      status
      value
      mergedSize
      error
    }
  }
`,Xe=`
  mutation mergeAppFileChunks($fileId: String!, $totalChunks: Int!, $fileName: String!, $totalSize: Long!) {
    mergeAppFileChunks(fileId: $fileId, totalChunks: $totalChunks, fileName: $fileName, totalSize: $totalSize) {
      status
      value
      mergedSize
      error
    }
  }
`,Ze=`
  mutation deleteChunks($fileId: String!) {
    deleteChunks(fileId: $fileId)
  }
`,Qe=`
  mutation startPomodoro($durationSec: Int!) {
    startPomodoro(durationSec: $durationSec)
  }
`,$e=`
  mutation stopPomodoro {
    stopPomodoro
  }
`,et=`
  mutation pausePomodoro {
    pausePomodoro
  }
`,tt=`
  mutation addBookmarks($urls: [String!]!, $groupId: ID!) {
    addBookmarks(urls: $urls, groupId: $groupId) {
      ...BookmarkFragment
    }
  }
  ${m}
`,nt=`
  mutation updateBookmark($id: ID!, $input: BookmarkInput!) {
    updateBookmark(id: $id, input: $input) {
      ...BookmarkFragment
    }
  }
  ${m}
`,rt=`
  mutation deleteBookmarks($ids: [ID!]!) {
    deleteBookmarks(ids: $ids) {
      affectedCount
    }
  }
`,it=`
  mutation recordBookmarkClick($id: ID!) {
    recordBookmarkClick(id: $id)
  }
`,at=`
  mutation createBookmarkGroup($name: String!) {
    createBookmarkGroup(name: $name) {
      ...BookmarkGroupFragment
    }
  }
  ${d}
`,ot=`
  mutation updateBookmarkGroup($id: ID!, $name: String!, $collapsed: Boolean!, $sortOrder: Int!) {
    updateBookmarkGroup(id: $id, name: $name, collapsed: $collapsed, sortOrder: $sortOrder) {
      ...BookmarkGroupFragment
    }
  }
  ${d}
`,st=`
  mutation deleteBookmarkGroup($id: ID!) {
    deleteBookmarkGroup(id: $id)
  }
`,ct=`
  mutation deleteFiles($paths: [String!]!) {
    deleteFiles(paths: $paths) {
      affectedCount
    }
  }
`,lt=`
  mutation { importImageSearchModel }
`,ut=`
  mutation { enableImageSearch }
`,dt=`
  mutation { disableImageSearch }
`,ft=`
  mutation { cancelImageModelDownload }
`,pt=`
  mutation startImageIndex($force: Boolean) {
    startImageIndex(force: $force)
  }
`,mt=`
  mutation { cancelImageIndex }
`,ht=`
  mutation createContact($input: ContactInput!) {
    createContact(input: $input) {
      ...ContactFragment
    }
  }
  ${p}
`,gt=`
  mutation updateContact($id: ID!, $input: ContactInput!) {
    updateContact(id: $id, input: $input) {
      ...ContactFragment
    }
  }
  ${p}
`,_t=`
  mutation DeleteNote($query: String!) {
    deleteNotes(query: $query) {
      affectedCount
    }
  }
`,vt=`
  mutation deleteFeedEntry($query: String!) {
    deleteFeedEntries(query: $query) {
      affectedCount
    }
  }
`,yt=`
  mutation removePref($key: String!) {
    removePref(key: $key)
  }
`,bt=`
  mutation DeleteDbTableRows($table: String!, $ids: [String!]!) {
    deleteDbTableRows(table: $table, ids: $ids)
  }
`,xt=`
  mutation AcquireDiscovery($token: ID!) {
    acquireDiscovery(token: $token)
  }
`,St=`
  mutation ReleaseDiscovery($token: ID!) {
    releaseDiscovery(token: $token)
  }
`,Ct=`
  mutation pairDevice($input: PairingDeviceInput!) {
    pairDevice(input: $input) { deviceId generation mode phase error }
  }
`,wt=`
  mutation cancelPairing($generation: String!) {
    cancelPairing(generation: $generation)
  }
`,Tt=`
  mutation respondToPairing($input: PairingRequestInput!, $accepted: Boolean!) {
    respondToPairing(input: $input, accepted: $accepted)
  }
`,Et=`
  mutation downloadPeerFile($messageId: ID!, $peerId: ID!) {
    downloadPeerFile(messageId: $messageId, peerId: $peerId)
  }
`,$=`
  mutation pauseDownload($messageId: ID!) {
    pauseDownload(messageId: $messageId)
  }
`,Dt=`
  mutation resumeDownload($messageId: ID!, $peerId: ID!) {
    resumeDownload(messageId: $messageId, peerId: $peerId)
  }
`,Ot=`
  mutation retryDownload($messageId: ID!, $peerId: ID!) {
    retryDownload(messageId: $messageId, peerId: $peerId)
  }
`,kt=`
  mutation pauseMediaScan { pauseMediaScan }
`,At=`
  mutation resumeMediaScan { resumeMediaScan }
`,jt=`
  mutation stopMediaScan { stopMediaScan }
`,Mt=`
  mutation rebuildMediaIndex($root: String!) { rebuildMediaIndex(root: $root) }
`,Nt=`
  mutation formatDisk($path: String!) {
    formatDisk(path: $path)
  }
`,Pt=`
  mutation setSambaSettings($input: SambaSettingsInput!) {
    setSambaSettings(input: $input)
  }
`,Ft=`
  mutation setSambaUserPassword($password: String!) {
    setSambaUserPassword(password: $password)
  }
`,It=`
  mutation advanceAudioSelection($action: AudioSelectionAction!, $expectedPath: String!, $expectedRevision: Long!) {
    advanceAudioSelection(action: $action, expectedPath: $expectedPath, expectedRevision: $expectedRevision) {
      track { ...AudioItemFragment }
      path
      positionMs
      revision
    }
  }
  ${a}
`;export{v as $,Be as $t,x as A,Tt as At,_t as B,z as Bt,oe as C,ue as Ct,ve as D,K as Dt,rt as E,F as Et,_e as F,At as Ft,ce as G,Me as Gt,We as H,Le as Ht,vt as I,S as It,ut as J,Pt as Jt,dt as K,de as Kt,De as L,Ot as Lt,Ke as M,ge as Mt,ye as N,Q as Nt,T as O,Ge as Ot,bt as P,Dt as Pt,Nt as Q,Qe as Qt,ct as R,te as Rt,fe as S,P as Sn,A as St,st as T,yt as Tt,E as U,Ne as Ut,me as V,y as Vt,ne as W,Pe as Wt,Se as X,Ue as Xt,V as Y,Ft as Yt,Ce as Z,pt as Zt,at as _,Oe as _n,it as _t,k as a,J as an,Xe as at,N as b,se as bn,St as bt,It as c,Ie as cn,X as ct,mt as d,H as dn,Ct as dt,jt as en,xe as et,ft as f,nt as fn,$ as ft,I as g,_ as gn,Mt as gt,G as h,gt as hn,B as ht,tt as i,ke as in,O as it,Ze as j,Y as jt,b as k,Ve as kt,Fe as l,Re as ln,Te as lt,h as m,w as mn,et as mt,xt as n,He as nn,ee as nt,le as o,he as on,Ye as ot,wt as p,ot as pn,kt as pt,Et as q,g as qt,W as r,Ae as rn,ze as rt,ie as s,Z as sn,L as st,j as t,$e as tn,lt as tt,je as u,D as un,Ee as ut,C as v,pe as vn,R as vt,M as w,re as wt,be as x,ae as xn,U as xt,ht as y,qe as yn,we as yt,q as z,Je as zt};