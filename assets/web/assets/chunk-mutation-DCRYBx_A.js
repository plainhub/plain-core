import{_ as e}from"./chunk-reactivity.esm-bundler-Dh02ET-1.js";import{t}from"./chunk-eventbus-Ca-q0d_H.js";import{a as n,t as r}from"./chunk-gql-client-CVJEveuU.js";import{At as i,Dt as a,Ft as o,It as s,Lt as c,Nt as l,Ot as u,Pt as d,Rt as f,jt as p,kt as m}from"./chunk-query-DRqDE_A4.js";function ee(i,a=!0){let o=e(!1),s=[],c=[];async function l(e){o.value=!0;try{let o=await n(i.document,e,{dedupe:!1});if(o.errors?.length){let e=o.errors[0].message;a&&t.emit(`toast`,e);let n=new r(e);for(let e of c)e(n);return}for(let e of s)e(o);return o}catch(e){if(e instanceof r&&e.canceled)return;let n=e instanceof r?e.message:`network_error`;a&&t.emit(`toast`,n);for(let t of c)t(e);return}finally{o.value=!1}}function u(e){return s.push(e),{off:()=>{let t=s.indexOf(e);t>=0&&s.splice(t,1)}}}function d(e){return c.push(e),{off:()=>{let t=c.indexOf(e);t>=0&&c.splice(t,1)}}}return{mutate:l,loading:o,onDone:u,onError:d}}async function te(e,t){return await e(t)!=null}var h=`
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
  ${p}
`,y=`
  mutation sendChatItems($targets: [String!]!, $content: ChatContentInput!) {
    sendChatItems(targets: $targets, content: $content) {
      ...ChatItemFragment
    }
  }
  ${p}
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
  ${p}
`,C=`
  mutation createChatChannel($name: String!) {
    createChatChannel(name: $name) {
      ...ChatChannelFragment
    }
  }
  ${i}
`,w=`
  mutation updateChatChannel($id: ID!, $name: String!) {
    updateChatChannel(id: $id, name: $name) {
      ...ChatChannelFragment
    }
  }
  ${i}
`,T=`
  mutation deleteChatChannel($id: ID!) {
    deleteChatChannel(id: $id)
  }
`,E=`
  mutation deletePeer($peerId: ID!) {
    deletePeer(peerId: $peerId)
  }
`,D=`
  mutation unpairPeer($peerId: ID!) {
    unpairPeer(peerId: $peerId)
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
  ${i}
`,A=`
  mutation removeChatChannelMember($id: ID!, $peerId: ID!) {
    removeChatChannelMember(id: $id, peerId: $peerId) {
      ...ChatChannelFragment
    }
  }
  ${i}
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
  ${s}
`,P=`
  mutation writeTextFile($path: String!, $content: String!, $overwrite: Boolean!) {
    writeTextFile(path: $path, content: $content, overwrite: $overwrite) {
      ...FileFragment
    }
  }
  ${s}
`,F=`
  mutation renameFile($path: String!, $name: String!) {
    renameFile(path: $path, name: $name)
  }
`,I=`
  mutation copyFileToDirectory($src: String!, $directory: String!, $name: String!) {
    copyFileToDirectory(src: $src, directory: $directory, name: $name)
  }
`,L=`
  mutation createFileTask($type: FileTaskType!, $title: String!, $ops: [FileTaskOpInput!]!) {
    createFileTask(type: $type, title: $title, ops: $ops, waitForCompletion: true) {
      status
      error
    }
  }
`,R=`
  mutation recordMediaPlay($mediaId: ID!, $type: PlaybackType!, $positionMs: Long!) {
    recordMediaPlay(mediaId: $mediaId, type: $type, positionMs: $positionMs)
  }
`,z=`
  mutation saveMediaPlayProgress($mediaId: ID!, $type: PlaybackType!, $positionMs: Long!) {
    saveMediaPlayProgress(mediaId: $mediaId, type: $type, positionMs: $positionMs)
  }
`,B=`
  mutation selectAudio($mediaId: ID!) {
    selectAudio(mediaId: $mediaId) {
      ...AudioItemFragment
    }
  }
  ${a}
`,V=`
  mutation enqueueAndSelectAudio($mediaId: ID!, $query: String!) {
    addAudioToQueue(query: $query)
    selectAudio(mediaId: $mediaId) {
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
  ${f}
`,se=`
  mutation updateTag($id: ID!, $name: String!) {
    updateTag(id: $id, name: $name) {
      ...TagFragment
    }
  }
  ${f}
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
  mutation setMountAlias($id: ID!, $alias: String!) {
    setMountAlias(id: $id, alias: $alias)
  }
`,fe=`
  mutation setFavoriteFolderAlias($fullPath: String!, $alias: String!) {
    setFavoriteFolderAlias(fullPath: $fullPath, alias: $alias) {
      rootPath
      fullPath
      alias
    }
  }
`,pe=`
  mutation createNote($input: NoteInput!) {
    createNote(input: $input) {
      ...NoteFragment
    }
  }
  ${c}
`,me=`
  mutation updateNote($id: ID!, $input: NoteInput!) {
    updateNote(id: $id, input: $input) {
      ...NoteFragment
    }
  }
  ${c}
`,he=`
  mutation deleteNotes($query: String!) {
    deleteNotes(query: $query) {
      affectedCount
    }
  }
`,ge=`
  mutation trashNotes($query: String!) {
    trashNotes(query: $query) {
      affectedCount
    }
  }
`,_e=`
  mutation restoreNotes($query: String!) {
    restoreNotes(query: $query) {
      affectedCount
    }
  }
`,ve=`
  mutation deleteFeedEntries($query: String!) {
    deleteFeedEntries(query: $query) {
      affectedCount
    }
  }
`,ye=`
  mutation deleteCalls($query: String!) {
    deleteCalls(query: $query) {
      affectedCount
    }
  }
`,be=`
  mutation deleteContacts($query: String!) {
    deleteContacts(query: $query) {
      affectedCount
    }
  }
`,xe=`
  mutation createFeed($url: String!, $fetchContent: Boolean!) {
    createFeed(url: $url, fetchContent: $fetchContent) {
      ...FeedFragment
    }
  }
  ${o}
`,Se=`
  mutation importFeeds($content: String!) {
    importFeeds(content: $content)
  }
`,Ce=`
  mutation relaunchApp {
    relaunchApp
  }
`,we=`
  mutation openAccessibilitySettings {
    openAccessibilitySettings
  }
`,Te=`
  mutation requestAccess($feature: AccessFeature!) {
    requestAccess(feature: $feature)
  }
`,Ee=`
  mutation deleteFeed($id: ID!) {
    deleteFeed(id: $id)
  }
`,De=`
  mutation updateFeed($id: ID!, $name: String!, $fetchContent: Boolean!) {
    updateFeed(id: $id, name: $name, fetchContent: $fetchContent) {
      ...FeedFragment
    }
  }
  ${o}
`,Oe=`
  mutation syncFeeds($id: ID) {
    syncFeeds(id: $id)
  }
`,ke=`
  mutation syncFeedEntryContent($id: ID!) {
    syncFeedEntryContent(id: $id) {
      ...FeedEntryFragment
      feed {
        ...FeedFragment
      }
    }
  }
  ${o}
  ${d}
`,Ae=`
  mutation call($number: String!, $showDialer: Boolean!) {
    call(number: $number, showDialer: $showDialer)
  }
`,je=`
  mutation setClipboard($text: String!) {
    setClipboard(text: $text)
  }
`,Me=`
  mutation sendSms($number: String!, $body: String!, $attachmentPaths: [String!]! = [], $subscriptionId: Int, $requestId: String!) {
    sendSms(number: $number, body: $body, attachmentPaths: $attachmentPaths, subscriptionId: $subscriptionId, requestId: $requestId) {
      requestId
      requiresConfirmation
    }
  }
`,Ne=`
  mutation archiveSmsConversation($id: String!) {
    archiveSmsConversation(id: $id)
  }
`,Pe=`
  mutation unarchiveSmsConversation($id: String!) {
    unarchiveSmsConversation(id: $id)
  }
`,Fe=`
  mutation uninstallPackages($id: ID!) {
    uninstallPackages(ids: [$id])
  }
`,Ie=`
  mutation installPackage($path: String!) {
    installPackage(path: $path) {
      id
      updatedAt
      isNew
    }
  }
`,Le=`
  mutation startScreenMirror($audio: Boolean!) {
    startScreenMirror(audio: $audio)
  }
`,Re=`
  mutation requestScreenMirrorAudio {
    requestScreenMirrorAudio
  }
`,ze=`
  mutation stopScreenMirror {
    stopScreenMirror
  }
`,Be=`
  mutation setTempValue($key: String!, $value: String!) {
    setTempValue(key: $key, value: $value) {
      key
      value
    }
  }
`,Ve=`
  mutation deleteNotifications($ids: [ID!]!) {
    deleteNotifications(ids: $ids) {
      affectedCount
    }
  }
`,He=`
  mutation replyNotification($id: ID!, $actionIndex: Int!, $text: String!) {
    replyNotification(id: $id, actionIndex: $actionIndex, text: $text)
  }
`,Ue=`
  mutation deleteClipboardItems($query: String!) {
    deleteClipboardItems(query: $query) {
      affectedCount
    }
  }
`,We=`
  mutation updateScreenMirrorQuality($mode: ScreenMirrorMode!) {
    updateScreenMirrorQuality(mode: $mode)
  }
`,Ge=`
  mutation saveFeedEntriesToNotes($query: String!) {
    saveFeedEntriesToNotes(query: $query)
  }
`,Ke=`
  mutation mergeChunks($fileId: String!, $totalChunks: Int!, $path: String!, $replace: Boolean!, $totalSize: Long!) {
    mergeChunks(fileId: $fileId, totalChunks: $totalChunks, path: $path, replace: $replace, totalSize: $totalSize) {
      status
      value
      mergedSize
      error
    }
  }
`,qe=`
  mutation mergeAppFileChunks($fileId: String!, $totalChunks: Int!, $fileName: String!, $totalSize: Long!) {
    mergeAppFileChunks(fileId: $fileId, totalChunks: $totalChunks, fileName: $fileName, totalSize: $totalSize) {
      status
      value
      mergedSize
      error
    }
  }
`,Je=`
  mutation deleteChunks($fileId: String!) {
    deleteChunks(fileId: $fileId)
  }
`,Ye=`
  mutation startPomodoro($durationSec: Int!) {
    startPomodoro(durationSec: $durationSec) { date completedCount currentRound timeLeftSec totalTimeSec isRunning isPaused state }
  }
`,Xe=`
  mutation stopPomodoro {
    stopPomodoro { date completedCount currentRound timeLeftSec totalTimeSec isRunning isPaused state }
  }
`,Ze=`
  mutation pausePomodoro {
    pausePomodoro { date completedCount currentRound timeLeftSec totalTimeSec isRunning isPaused state }
  }
`,Qe=`mutation logout { logout }`,$e=`
  mutation importBookmarks($content: String!) {
    importBookmarks(content: $content) { affectedCount }
  }
`,et=`
  mutation addBookmarks($urls: [String!]!, $groupId: ID!) {
    addBookmarks(urls: $urls, groupId: $groupId) {
      ...BookmarkFragment
    }
  }
  ${u}
`,tt=`
  mutation updateBookmark($id: ID!, $input: BookmarkInput!) {
    updateBookmark(id: $id, input: $input) {
      ...BookmarkFragment
    }
  }
  ${u}
`,nt=`
  mutation deleteBookmarks($ids: [ID!]!) {
    deleteBookmarks(ids: $ids) {
      affectedCount
    }
  }
`,rt=`
  mutation recordBookmarkClick($id: ID!) {
    recordBookmarkClick(id: $id)
  }
`,it=`
  mutation createBookmarkGroup($name: String!) {
    createBookmarkGroup(name: $name) {
      ...BookmarkGroupFragment
    }
  }
  ${m}
`,at=`
  mutation updateBookmarkGroup($id: ID!, $name: String!, $collapsed: Boolean!, $sortOrder: Int!) {
    updateBookmarkGroup(id: $id, name: $name, collapsed: $collapsed, sortOrder: $sortOrder) {
      ...BookmarkGroupFragment
    }
  }
  ${m}
`,ot=`
  mutation deleteBookmarkGroup($id: ID!) {
    deleteBookmarkGroup(id: $id)
  }
`,st=`
  mutation deleteFiles($paths: [String!]!) {
    deleteFiles(paths: $paths) {
      affectedCount
    }
  }
`,ct=`
  mutation { importImageSearchModel }
`,lt=`
  mutation { enableImageSearch }
`,ut=`
  mutation { disableImageSearch }
`,dt=`
  mutation { cancelImageModelDownload }
`,ft=`
  mutation startImageIndex($force: Boolean) {
    startImageIndex(force: $force)
  }
`,pt=`
  mutation { cancelImageIndex }
`,mt=`
  mutation createContact($input: ContactInput!) {
    createContact(input: $input) {
      ...ContactFragment
    }
  }
  ${l}
`,ht=`
  mutation updateContact($id: ID!, $input: ContactInput!) {
    updateContact(id: $id, input: $input) {
      ...ContactFragment
    }
  }
  ${l}
`,gt=`
  mutation DeleteNote($query: String!) {
    deleteNotes(query: $query) {
      affectedCount
    }
  }
`,_t=`
  mutation deleteFeedEntry($query: String!) {
    deleteFeedEntries(query: $query) {
      affectedCount
    }
  }
`,vt=`
  mutation removePref($key: String!) {
    removePref(key: $key)
  }
`,yt=`
  mutation DeleteDbTableRows($table: String!, $ids: [String!]!) {
    deleteDbTableRows(table: $table, ids: $ids)
  }
`,$=`
  mutation StartDiscovery {
    startDiscovery
  }
`,bt=`
  mutation StopDiscovery {
    stopDiscovery
  }
`,xt=`
  mutation PairDevice($input: PairPeerInput!) {
    pairPeer(input: $input)
  }
`,St=`
  mutation cancelPairing($peerId: ID!) {
    cancelPairing(peerId: $peerId)
  }
`,Ct=`
  mutation respondToPairing($peerId: ID!, $accepted: Boolean!) {
    respondToPairing(peerId: $peerId, accepted: $accepted)
  }
`,wt=`
  mutation createDownload($input: DownloadInput!) {
    createDownload(input: $input) { id }
  }
`,Tt=`
  mutation pauseDownload($id: ID!) { pauseDownload(id: $id) }
`,Et=`
  mutation resumeDownload($id: ID!) { resumeDownload(id: $id) }
`,Dt=`
  mutation retryDownload($id: ID!) { retryDownload(id: $id) }
`,Ot=`
  mutation pauseFileIndex($volumeId: ID) { pauseFileIndex(volumeId: $volumeId) }
`,kt=`
  mutation resumeFileIndex($volumeId: ID) { resumeFileIndex(volumeId: $volumeId) }
`,At=`
  mutation stopFileIndex($volumeId: ID) { stopFileIndex(volumeId: $volumeId) }
`,jt=`
  mutation formatDisk($path: String!) {
    formatDisk(path: $path)
  }
`;export{ct as $,Oe as $t,x as A,kt as At,gt as B,fe as Bt,oe as C,Te as Ct,ye as D,_e as Dt,nt as E,Y as Et,ve as F,z as Ft,ce as G,ft as Gt,Ve as H,g as Ht,_t as I,B as It,V as J,bt as Jt,ut as K,Ye as Kt,Ee as L,y as Lt,Ue as M,Dt as Mt,be as N,te as Nt,T as O,Q as Ot,yt as P,Ge as Pt,Se as Q,ke as Qt,st as R,Me as Rt,pe as S,He as St,ot as T,Ct as Tt,E as U,Be as Ut,he as V,de as Vt,ne as W,$ as Wt,v as X,Xe as Xt,jt as Y,At as Yt,$e as Z,ze as Zt,mt as _,P as _n,ue as _t,le as a,D as an,Ke as at,xe as b,F as bt,Ae as c,at as cn,xt as ct,St as d,_ as dn,Ze as dt,J as en,ee as et,h as f,De as fn,rt as ft,C as g,ae as gn,A as gt,it as h,se as hn,U as ht,k as i,Fe as in,qe as it,Je as j,S as jt,b as k,Et as kt,pt as l,w as ln,Tt as lt,I as m,We as mn,Ce as mt,W as n,Z as nn,O as nt,ie as o,H as on,X as ot,G as p,me as pn,R as pt,lt as q,Le as qt,et as r,Pe as rn,Qe as rt,Ne as s,tt as sn,we as st,j as t,ge as tn,Ie as tt,dt as u,ht as un,Ot as ut,N as v,re as vt,M as w,Re as wt,L as x,K as xt,wt as y,vt as yt,q as z,je as zt};