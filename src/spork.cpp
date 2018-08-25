// Copyright (c) 2014-2018 The Dash Core developers
// Distributed under the MIT software license, see the accompanying
// file COPYING or http://www.opensource.org/licenses/mit-license.php.

#include "spork.h"

#include "base58.h"
#include "chainparams.h"
#include "validation.h"
#include "messagesigner.h"
#include "net_processing.h"
#include "netmessagemaker.h"

#include <string>

CSporkManager sporkManager;

std::map<int, int64_t> mapSporkDefaults = {
    {SPORK_2_INSTANTSEND_ENABLED,            0},             // ON
    {SPORK_3_INSTANTSEND_BLOCK_FILTERING,    0},             // ON
    {SPORK_5_INSTANTSEND_MAX_VALUE,          1000},          // 1000 Dash
    {SPORK_6_NEW_SIGS,                       4070908800ULL}, // OFF
    {SPORK_8_MASTERNODE_PAYMENT_ENFORCEMENT, 4070908800ULL}, // OFF
    {SPORK_9_SUPERBLOCKS_ENABLED,            4070908800ULL}, // OFF
    {SPORK_10_MASTERNODE_PAY_UPDATED_NODES,  4070908800ULL}, // OFF
    {SPORK_12_RECONSIDER_BLOCKS,             0},             // 0 BLOCKS
    {SPORK_14_REQUIRE_SENTINEL_FLAG,         4070908800ULL}, // OFF
};

void CSporkManager::Clear()
{
    LOCK(cs);
    mapLegacySporksActive.clear();
    mapLegacySporksByHash.clear();
    vecSporkKeyIDs.clear();
    // TODO:
    // legacySporkPubKeyID.SetNull();
    sporkPrivKey = CKey();
}

void CSporkManager::ProcessSpork(CNode* pfrom, const std::string& strCommand, CDataStream& vRecv, CConnman& connman)
{
    if(fLiteMode) return; // disable all Dash specific functionality

    if (strCommand == NetMsgType::SPORK) {

        CSporkMessage spork;
        vRecv >> spork;

        uint256 hash = spork.GetHash();

        {
            LOCK(cs_main);
            pfrom->setAskFor.erase(hash);
            if(!chainActive.Tip()) return;
        }
        {
            LOCK(cs); // make sure to not lock this together with cs_main

            // Get signer CKeyID from spork message signature.
            // CKeyID signerId = spork.GetSignerKeyID();

            CKeyID signerId = CKeyID();
            if (!spork.GetSignerKeyID(signerId)) {
                LogPrintf("CSporkManager::ProcessSpork -- ERROR: unable to recover key from signature\n");
                return;
            }

            bool fLegacySporkKey = (signerId == legacySporkPubKeyID);

            // If a newer spork already exists in map, skip the incoming.
            // if signed by legacy spork key
            if (fLegacySporkKey &&
                mapLegacySporksActive.count(spork.nSporkID) &&
                mapLegacySporksActive[spork.nSporkID].nTimeSigned >= spork.nTimeSigned) {
                return;
            }
            // else
            else if (!fLegacySporkKey &&
                mapSporksActive.count(spork.nSporkID) &&
                mapSporksActive[spork.nSporkID].count(signerId) &&
                mapSporksActive[spork.nSporkID][signerId].nTimeSigned >= spork.nTimeSigned) {
                return;
            }
        }

        // TODO: multisig spork logic here
        // std::map<int, std::map<CKeyID, CSporkMessage> > mapSporksActive;

        bool fFoundValidSignature = false;
        for (const auto& keyId : vecSporkKeyIDs) {
            if (spork.CheckSignature(keyId, IsSporkActive(SPORK_6_NEW_SIGS))) {
                fFoundValidSignature = true;
                break;
            }
        }
        if(!fFoundValidSignature) {
            LOCK(cs_main);
            LogPrintf("CSporkManager::ProcessSpork -- ERROR: invalid signature\n");
            Misbehaving(pfrom->GetId(), 100);
            return;
        }

        {
            // TODO: ensure threshold keys have signed first
            // std::map<int, CSporkMessage> mapSporksActive;
            // std::map<> mapKeys = mapSporksActive[spork.nSporkID];
            LOCK(cs); // make sure to not lock this together with cs_main
            mapLegacySporksByHash[hash] = spork;
            mapLegacySporksActive[spork.nSporkID] = spork;
        }
        spork.Relay(connman);

        //does a task if needed
        // TODO: ensure threshold keys have signed first
        ExecuteSpork(spork.nSporkID, spork.nValue);

    } else if (strCommand == NetMsgType::GETSPORKS) {
        LOCK(cs); // make sure to not lock this together with cs_main
        for (const auto& pair : mapLegacySporksActive) {
            connman.PushMessage(pfrom, CNetMsgMaker(pfrom->GetSendVersion()).Make(NetMsgType::SPORK, pair.second));
        }
    }

}

void CSporkManager::ExecuteSpork(int nSporkID, int nValue)
{
    //correct fork via spork technology
    if(nSporkID == SPORK_12_RECONSIDER_BLOCKS && nValue > 0) {
        // allow to reprocess 24h of blocks max, which should be enough to resolve any issues
        int64_t nMaxBlocks = 576;
        // this potentially can be a heavy operation, so only allow this to be executed once per 10 minutes
        int64_t nTimeout = 10 * 60;

        static int64_t nTimeExecuted = 0; // i.e. it was never executed before

        if(GetTime() - nTimeExecuted < nTimeout) {
            LogPrint("spork", "CSporkManager::ExecuteSpork -- ERROR: Trying to reconsider blocks, too soon - %d/%d\n", GetTime() - nTimeExecuted, nTimeout);
            return;
        }

        if(nValue > nMaxBlocks) {
            LogPrintf("CSporkManager::ExecuteSpork -- ERROR: Trying to reconsider too many blocks %d/%d\n", nValue, nMaxBlocks);
            return;
        }


        LogPrintf("CSporkManager::ExecuteSpork -- Reconsider Last %d Blocks\n", nValue);

        ReprocessBlocks(nValue);
        nTimeExecuted = GetTime();
    }
}

bool CSporkManager::UpdateSpork(int nSporkID, int64_t nValue, CConnman& connman)
{
    CSporkMessage spork = CSporkMessage(nSporkID, nValue, GetAdjustedTime());

    if(spork.Sign(sporkPrivKey, IsSporkActive(SPORK_6_NEW_SIGS))) {
        spork.Relay(connman);
        LOCK(cs);
        mapLegacySporksByHash[spork.GetHash()] = spork;
        mapLegacySporksActive[nSporkID] = spork;
        return true;
    }

    return false;
}

// grab the spork, otherwise say it's off
bool CSporkManager::IsSporkActive(int nSporkID)
{
    LOCK(cs);
    int64_t r = -1;

    if(mapLegacySporksActive.count(nSporkID)){
        r = mapLegacySporksActive[nSporkID].nValue;
    } else if (mapSporkDefaults.count(nSporkID)) {
        r = mapSporkDefaults[nSporkID];
    } else {
        LogPrint("spork", "CSporkManager::IsSporkActive -- Unknown Spork ID %d\n", nSporkID);
        r = 4070908800ULL; // 2099-1-1 i.e. off by default
    }

    return r < GetAdjustedTime();
}

// grab the value of the spork on the network, or the default
int64_t CSporkManager::GetSporkValue(int nSporkID)
{
    LOCK(cs);
    // TODO: update to pull value with threshold sigs
    if (mapLegacySporksActive.count(nSporkID))
        return mapLegacySporksActive[nSporkID].nValue;

    if (mapSporkDefaults.count(nSporkID)) {
        return mapSporkDefaults[nSporkID];
    }

    LogPrint("spork", "CSporkManager::GetSporkValue -- Unknown Spork ID %d\n", nSporkID);
    return -1;
}

int CSporkManager::GetSporkIDByName(const std::string& strName)
{
    if (strName == "SPORK_2_INSTANTSEND_ENABLED")               return SPORK_2_INSTANTSEND_ENABLED;
    if (strName == "SPORK_3_INSTANTSEND_BLOCK_FILTERING")       return SPORK_3_INSTANTSEND_BLOCK_FILTERING;
    if (strName == "SPORK_5_INSTANTSEND_MAX_VALUE")             return SPORK_5_INSTANTSEND_MAX_VALUE;
    if (strName == "SPORK_6_NEW_SIGS")                          return SPORK_6_NEW_SIGS;
    if (strName == "SPORK_8_MASTERNODE_PAYMENT_ENFORCEMENT")    return SPORK_8_MASTERNODE_PAYMENT_ENFORCEMENT;
    if (strName == "SPORK_9_SUPERBLOCKS_ENABLED")               return SPORK_9_SUPERBLOCKS_ENABLED;
    if (strName == "SPORK_10_MASTERNODE_PAY_UPDATED_NODES")     return SPORK_10_MASTERNODE_PAY_UPDATED_NODES;
    if (strName == "SPORK_12_RECONSIDER_BLOCKS")                return SPORK_12_RECONSIDER_BLOCKS;
    if (strName == "SPORK_14_REQUIRE_SENTINEL_FLAG")            return SPORK_14_REQUIRE_SENTINEL_FLAG;

    LogPrint("spork", "CSporkManager::GetSporkIDByName -- Unknown Spork name '%s'\n", strName);
    return -1;
}

std::string CSporkManager::GetSporkNameByID(int nSporkID)
{
    switch (nSporkID) {
        case SPORK_2_INSTANTSEND_ENABLED:               return "SPORK_2_INSTANTSEND_ENABLED";
        case SPORK_3_INSTANTSEND_BLOCK_FILTERING:       return "SPORK_3_INSTANTSEND_BLOCK_FILTERING";
        case SPORK_5_INSTANTSEND_MAX_VALUE:             return "SPORK_5_INSTANTSEND_MAX_VALUE";
        case SPORK_6_NEW_SIGS:                          return "SPORK_6_NEW_SIGS";
        case SPORK_8_MASTERNODE_PAYMENT_ENFORCEMENT:    return "SPORK_8_MASTERNODE_PAYMENT_ENFORCEMENT";
        case SPORK_9_SUPERBLOCKS_ENABLED:               return "SPORK_9_SUPERBLOCKS_ENABLED";
        case SPORK_10_MASTERNODE_PAY_UPDATED_NODES:     return "SPORK_10_MASTERNODE_PAY_UPDATED_NODES";
        case SPORK_12_RECONSIDER_BLOCKS:                return "SPORK_12_RECONSIDER_BLOCKS";
        case SPORK_14_REQUIRE_SENTINEL_FLAG:            return "SPORK_14_REQUIRE_SENTINEL_FLAG";
        default:
            LogPrint("spork", "CSporkManager::GetSporkNameByID -- Unknown Spork ID %d\n", nSporkID);
            return "Unknown";
    }
}

bool CSporkManager::GetSporkByHash(const uint256& hash, CSporkMessage &sporkRet)
{
    LOCK(cs);

    const auto it = mapLegacySporksByHash.find(hash);

    if (it == mapLegacySporksByHash.end())
        return false;

    sporkRet = it->second;

    return true;
}

bool CSporkManager::SetSporkAddress(const std::string& strAddress)
{
    LOCK(cs);
    CBitcoinAddress address(strAddress);

    CKeyID h160;
    if (!address.IsValid() || !address.GetKeyID(h160)) {
        LogPrintf("CSporkManager::SetSporkAddress -- Failed to parse spork address\n");
        return false;
    }

    for (const auto& keyId : vecSporkKeyIDs) {
        if (h160 == keyId) {
            LogPrintf("CSporkManager::SetSporkAddress -- Spork Address %s already set\n", strAddress);
            return true;
        }
    }

    vecSporkKeyIDs.push_back(h160);

    return true;
}

bool CSporkManager::SetLegacySporkAddress(const std::string& strAddress)
{
    LOCK(cs);
    CBitcoinAddress address(strAddress);

    if (!address.IsValid() || !address.GetKeyID(legacySporkPubKeyID)) {
        LogPrintf("CSporkManager::SetSporkAddress -- Failed to parse spork address\n");
        return false;
    }

    return true;
}

bool CSporkManager::SetPrivKey(const std::string& strPrivKey)
{
    CKey key;
    CPubKey pubKey;
    if(!CMessageSigner::GetKeysFromSecret(strPrivKey, key, pubKey)) {
        LogPrintf("CSporkManager::SetPrivKey -- Failed to parse private key\n");
        return false;
    }

    bool fFoundMatchingKey = false;
    for (const auto& keyId : vecSporkKeyIDs) {
        if (pubKey.GetID() == keyId) {
            fFoundMatchingKey = true;
            break;
        }
    }
    if(!fFoundMatchingKey) {
        LogPrintf("CSporkManager::SetPrivKey -- New private key does not belong to spork address\n");
        return false;
    }

    CSporkMessage spork;
    if (spork.Sign(key, IsSporkActive(SPORK_6_NEW_SIGS))) {
        LOCK(cs);
        // Test signing successful, proceed
        LogPrintf("CSporkManager::SetPrivKey -- Successfully initialized as spork signer\n");

        sporkPrivKey = key;
        return true;
    } else {
        LogPrintf("CSporkManager::SetPrivKey -- Test signing failed\n");
        return false;
    }
}

std::string CSporkManager::ToString() const
{
    LOCK(cs);
    // TODO: this could probably give more info
    return strprintf("Sporks: %llu", mapLegacySporksActive.size());
}

bool CSporkManager::SetSignatureThreshold(const int& threshold)
{
    LOCK(cs);
    nSporkSigThreshold = threshold;

    return true;
}



uint256 CSporkMessage::GetHash() const
{
    return SerializeHash(*this);
}

uint256 CSporkMessage::GetSignatureHash() const
{
    return GetHash();
}

bool CSporkMessage::Sign(const CKey& key, bool fSporkSixActive)
{
    if (!key.IsValid()) {
        LogPrintf("CSporkMessage::Sign -- signing key is not valid\n");
        return false;
    }

    CKeyID pubKeyId = key.GetPubKey().GetID();
    std::string strError = "";

    if (fSporkSixActive) {
        uint256 hash = GetSignatureHash();

        if(!CHashSigner::SignHash(hash, key, vchSig)) {
            LogPrintf("CSporkMessage::Sign -- SignHash() failed\n");
            return false;
        }

        if (!CHashSigner::VerifyHash(hash, pubKeyId, vchSig, strError)) {
            LogPrintf("CSporkMessage::Sign -- VerifyHash() failed, error: %s\n", strError);
            return false;
        }
    } else {
        std::string strMessage = std::to_string(nSporkID) + std::to_string(nValue) + std::to_string(nTimeSigned);

        if(!CMessageSigner::SignMessage(strMessage, vchSig, key)) {
            LogPrintf("CSporkMessage::Sign -- SignMessage() failed\n");
            return false;
        }

        if(!CMessageSigner::VerifyMessage(pubKeyId, vchSig, strMessage, strError)) {
            LogPrintf("CSporkMessage::Sign -- VerifyMessage() failed, error: %s\n", strError);
            return false;
        }
    }

    return true;
}

bool CSporkMessage::CheckSignature(const CKeyID& pubKeyId, bool fSporkSixActive) const
{
    std::string strError = "";

    if (fSporkSixActive) {
        uint256 hash = GetSignatureHash();

        if (!CHashSigner::VerifyHash(hash, pubKeyId, vchSig, strError)) {
            // Note: unlike for many other messages when SPORK_6_NEW_SIGS is ON sporks with sigs in old format
            // and newer timestamps should not be accepted, so if we failed here - that's it
            LogPrintf("CSporkMessage::CheckSignature -- VerifyHash() failed, error: %s\n", strError);
            return false;
        }
    } else {
        std::string strMessage = std::to_string(nSporkID) + std::to_string(nValue) + std::to_string(nTimeSigned);

        if (!CMessageSigner::VerifyMessage(pubKeyId, vchSig, strMessage, strError)){
            // Note: unlike for other messages we have to check for new format even with SPORK_6_NEW_SIGS
            // inactive because SPORK_6_NEW_SIGS default is OFF and it is not the first spork to sync
            // (and even if it would, spork order can't be guaranteed anyway).
            uint256 hash = GetSignatureHash();
            if (!CHashSigner::VerifyHash(hash, pubKeyId, vchSig, strError)) {
                LogPrintf("CSporkMessage::CheckSignature -- VerifyHash() failed, error: %s\n", strError);
                return false;
            }
        }
    }

    return true;
}

void CSporkMessage::Relay(CConnman& connman)
{
    CInv inv(MSG_SPORK, GetHash());
    connman.RelayInv(inv);
}


bool CSporkMessage::GetSignerKeyID(CKeyID& sporkSignerID) {
    // TODO: extract hash160 from vchSig and return it

    CPubKey pubkeyFromSig;
    if(!pubkeyFromSig.RecoverCompact(GetHash(), vchSig)) {
        return false;
    }

    sporkSignerID = pubkeyFromSig.GetID();
    return true;
}
