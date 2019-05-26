// Copyright (c) 2014-2018 The Dash Core developers
// Distributed under the MIT/X11 software license, see the accompanying
// file COPYING or http://www.opensource.org/licenses/mit-license.php.

#include <iostream>
using std::cout;
using std::endl;

#include "governance-classes.h"
#include "core_io.h"
#include "init.h"
#include "utilstrencodings.h"
#include "validation.h"

// TODO: Remove once we no longer have to parse '|'-delimited strings.
#include <boost/algorithm/string.hpp>

#include <univalue.h>

// DECLARE GLOBAL VARIABLES FOR GOVERNANCE CLASSES
CGovernanceTriggerManager triggerman;

// TODO: Remove once we no longer have to parse '|'-delimited strings.
// SPLIT UP STRING BY DELIMITER
// http://www.boost.org/doc/libs/1_58_0/doc/html/boost/algorithm/split_idp202406848.html
std::vector<std::string> SplitBy(const std::string& strCommand, const std::string& strDelimit)
{
    std::vector<std::string> vParts;
    boost::split(vParts, strCommand, boost::is_any_of(strDelimit));

    for (int q = 0; q < (int)vParts.size(); q++) {
        if (strDelimit.find(vParts[q]) != std::string::npos) {
            vParts.erase(vParts.begin() + q);
            --q;
        }
    }

    return vParts;
}

// TODO: Remove once we no longer have to payments as strings.
CAmount ParsePaymentAmount(const std::string& strAmount)
{
    CAmount nAmount = 0;
    if (strAmount.empty()) {
        std::ostringstream ostr;
        ostr << "ParsePaymentAmount: Amount is empty";
        throw std::runtime_error(ostr.str());
    }
    if (strAmount.size() > 20) {
        // String is much too long, the functions below impose stricter
        // requirements
        std::ostringstream ostr;
        ostr << "ParsePaymentAmount: Amount string too long";
        throw std::runtime_error(ostr.str());
    }
    // Make sure the string makes sense as an amount
    // Note: No spaces allowed
    // Also note: No scientific notation
    size_t pos = strAmount.find_first_not_of("0123456789.");
    if (pos != std::string::npos) {
        std::ostringstream ostr;
        ostr << "ParsePaymentAmount: Amount string contains invalid character";
        throw std::runtime_error(ostr.str());
    }

    pos = strAmount.find(".");
    if (pos == 0) {
        // JSON doesn't allow values to start with a decimal point
        std::ostringstream ostr;
        ostr << "ParsePaymentAmount: Invalid amount string, leading decimal point not allowed";
        throw std::runtime_error(ostr.str());
    }

    // Make sure there's no more than 1 decimal point
    if ((pos != std::string::npos) && (strAmount.find(".", pos + 1) != std::string::npos)) {
        std::ostringstream ostr;
        ostr << "ParsePaymentAmount: Invalid amount string, too many decimal points";
        throw std::runtime_error(ostr.str());
    }

    // Note this code is taken from AmountFromValue in rpcserver.cpp
    // which is used for parsing the amounts in createrawtransaction.
    if (!ParseFixedPoint(strAmount, 8, &nAmount)) {
        nAmount = 0;
        std::ostringstream ostr;
        ostr << "ParsePaymentAmount: ParseFixedPoint failed for string: " << strAmount;
        throw std::runtime_error(ostr.str());
    }
    if (!MoneyRange(nAmount)) {
        nAmount = 0;
        std::ostringstream ostr;
        ostr << "ParsePaymentAmount: Invalid amount string, value outside of valid money range";
        throw std::runtime_error(ostr.str());
    }

    return nAmount;
}

/**
*   Add Governance Object
*/

bool CGovernanceTriggerManager::AddNewTrigger(uint256 nHash)
{
    AssertLockHeld(governance.cs);

    // IF WE ALREADY HAVE THIS HASH, RETURN
    if (mapTrigger.count(nHash)) {
        LogPrint(BCLog::GOBJECT, "CGovernanceTriggerManager::AddNewTrigger -- Already have hash, nHash = %s, count = %d, size = %s\n",
                    nHash.GetHex(), mapTrigger.count(nHash), mapTrigger.size());
        return false;
    }

    CSuperblock_sptr pSuperblock;
    try {
        CSuperblock_sptr pSuperblockTmp(new CSuperblock(nHash));
        pSuperblock = pSuperblockTmp;
    } catch (std::exception& e) {
        LogPrintf("CGovernanceTriggerManager::AddNewTrigger -- Error creating superblock: %s\n", e.what());
        return false;
    } catch (...) {
        LogPrintf("CGovernanceTriggerManager::AddNewTrigger: Unknown Error creating superblock\n");
        return false;
    }

    pSuperblock->SetStatus(SEEN_OBJECT_IS_VALID);

    mapTrigger.insert(std::make_pair(nHash, pSuperblock));

    return true;
}

/**
*
*   Clean And Remove
*
*/

void CGovernanceTriggerManager::CleanAndRemove()
{
    AssertLockHeld(governance.cs);

    // Remove triggers that are invalid or expired
    LogPrint(BCLog::GOBJECT, "CGovernanceTriggerManager::CleanAndRemove -- mapTrigger.size() = %d\n", mapTrigger.size());

    trigger_m_it it = mapTrigger.begin();
    while (it != mapTrigger.end()) {
        bool remove = false;
        CGovernanceObject* pObj = nullptr;
        CSuperblock_sptr& pSuperblock = it->second;
        if (!pSuperblock) {
            LogPrint(BCLog::GOBJECT, "CGovernanceTriggerManager::CleanAndRemove -- NULL superblock marked for removal\n");
            remove = true;
        } else {
            pObj = governance.FindGovernanceObject(it->first);
            if (!pObj || pObj->GetObjectType() != GOVERNANCE_OBJECT_TRIGGER) {
                LogPrint(BCLog::GOBJECT, "CGovernanceTriggerManager::CleanAndRemove -- Unknown or non-trigger superblock\n");
                pSuperblock->SetStatus(SEEN_OBJECT_ERROR_INVALID);
            }

            LogPrint(BCLog::GOBJECT, "CGovernanceTriggerManager::CleanAndRemove -- superblock status = %d\n", pSuperblock->GetStatus());
            switch (pSuperblock->GetStatus()) {
            case SEEN_OBJECT_ERROR_INVALID:
            case SEEN_OBJECT_UNKNOWN:
                LogPrint(BCLog::GOBJECT, "CGovernanceTriggerManager::CleanAndRemove -- Unknown or invalid trigger found\n");
                remove = true;
                break;
            case SEEN_OBJECT_IS_VALID:
            case SEEN_OBJECT_EXECUTED:
                remove = pSuperblock->IsExpired();
                break;
            default:
                break;
            }
        }
        LogPrint(BCLog::GOBJECT, "CGovernanceTriggerManager::CleanAndRemove -- %smarked for removal\n", remove ? "" : "NOT ");

        if (remove) {
            std::string strDataAsPlainString = "NULL";
            if (pObj) {
                strDataAsPlainString = pObj->GetDataAsPlainString();
            }
            LogPrint(BCLog::GOBJECT, "CGovernanceTriggerManager::CleanAndRemove -- Removing trigger object %s\n", strDataAsPlainString);
            // mark corresponding object for deletion
            if (pObj) {
                pObj->fCachedDelete = true;
                if (pObj->nDeletionTime == 0) {
                    pObj->nDeletionTime = GetAdjustedTime();
                }
            }
            // delete the trigger
            mapTrigger.erase(it++);
        } else {
            ++it;
        }
    }
}

/**
*   Get Active Triggers
*
*   - Look through triggers and scan for active ones
*   - Return the triggers in a list
*/

std::vector<CSuperblock_sptr> CGovernanceTriggerManager::GetActiveTriggers()
{
    AssertLockHeld(governance.cs);
    std::vector<CSuperblock_sptr> vecResults;

    // LOOK AT THESE OBJECTS AND COMPILE A VALID LIST OF TRIGGERS
    for (const auto& pair : mapTrigger) {
        CGovernanceObject* pObj = governance.FindGovernanceObject(pair.first);
        if (pObj) {
            vecResults.push_back(pair.second);
        }
    }

    return vecResults;
}

/**
*   Is Superblock Triggered
*
*   - Does this block have a non-executed and actived trigger?
*/

bool CSuperblockManager::IsSuperblockTriggered(int nBlockHeight)
{
    LogPrint(BCLog::GOBJECT, "CSuperblockManager::IsSuperblockTriggered -- Start nBlockHeight = %d\n", nBlockHeight);
    if (!CSuperblock::IsValidBlockHeight(nBlockHeight)) {
        return false;
    }

    LOCK(governance.cs);
    // GET ALL ACTIVE TRIGGERS
    std::vector<CSuperblock_sptr> vecTriggers = triggerman.GetActiveTriggers();

    LogPrint(BCLog::GOBJECT, "CSuperblockManager::IsSuperblockTriggered -- vecTriggers.size() = %d\n", vecTriggers.size());

    for (const auto& pSuperblock : vecTriggers) {
        if (!pSuperblock) {
            LogPrintf("CSuperblockManager::IsSuperblockTriggered -- Non-superblock found, continuing\n");
            continue;
        }

        CGovernanceObject* pObj = pSuperblock->GetGovernanceObject();

        if (!pObj) {
            LogPrintf("CSuperblockManager::IsSuperblockTriggered -- pObj == nullptr, continuing\n");
            continue;
        }

        LogPrint(BCLog::GOBJECT, "CSuperblockManager::IsSuperblockTriggered -- data = %s\n", pObj->GetDataAsPlainString());

        // note : 12.1 - is epoch calculation correct?

        if (nBlockHeight != pSuperblock->GetBlockHeight()) {
            LogPrint(BCLog::GOBJECT, "CSuperblockManager::IsSuperblockTriggered -- block height doesn't match nBlockHeight = %d, blockStart = %d, continuing\n",
                nBlockHeight,
                pSuperblock->GetBlockHeight());
            continue;
        }

        // MAKE SURE THIS TRIGGER IS ACTIVE VIA FUNDING CACHE FLAG

        pObj->UpdateSentinelVariables();

        if (pObj->IsSetCachedFunding()) {
            LogPrint(BCLog::GOBJECT, "CSuperblockManager::IsSuperblockTriggered -- fCacheFunding = true, returning true\n");
            return true;
        } else {
            LogPrint(BCLog::GOBJECT, "CSuperblockManager::IsSuperblockTriggered -- fCacheFunding = false, continuing\n");
        }
    }

    return false;
}


bool CSuperblockManager::GetBestSuperblock(CSuperblock_sptr& pSuperblockRet, int nBlockHeight)
{
    if (!CSuperblock::IsValidBlockHeight(nBlockHeight)) {
        return false;
    }

    AssertLockHeld(governance.cs);
    std::vector<CSuperblock_sptr> vecTriggers = triggerman.GetActiveTriggers();
    int nYesCount = 0;

    for (const auto& pSuperblock : vecTriggers) {
        if (!pSuperblock || nBlockHeight != pSuperblock->GetBlockHeight()) {
            continue;
        }

        CGovernanceObject* pObj = pSuperblock->GetGovernanceObject();

        if (!pObj) {
            continue;
        }

        // DO WE HAVE A NEW WINNER?

        int nTempYesCount = pObj->GetAbsoluteYesCount(VOTE_SIGNAL_FUNDING);
        if (nTempYesCount > nYesCount) {
            nYesCount = nTempYesCount;
            pSuperblockRet = pSuperblock;
        }
    }

    return nYesCount > 0;
}

/**
*   Get Superblock Payments
*
*   - Returns payments for superblock
*/

bool CSuperblockManager::GetSuperblockPayments(int nBlockHeight, std::vector<CTxOut>& voutSuperblockRet)
{
    LOCK(governance.cs);

    // GET THE BEST SUPERBLOCK FOR THIS BLOCK HEIGHT

    CSuperblock_sptr pSuperblock;
    if (!CSuperblockManager::GetBestSuperblock(pSuperblock, nBlockHeight)) {
        LogPrint(BCLog::GOBJECT, "CSuperblockManager::GetSuperblockPayments -- Can't find superblock for height %d\n", nBlockHeight);
        return false;
    }

    // make sure it's empty, just in case
    voutSuperblockRet.clear();

    // GET SUPERBLOCK OUTPUTS

    // Superblock payments will be appended to the end of the coinbase vout vector

    // TODO: How many payments can we add before things blow up?
    //       Consider at least following limits:
    //          - max coinbase tx size
    //          - max "budget" available
    for (int i = 0; i < pSuperblock->CountPayments(); i++) {
        CGovernancePayment payment;
        if (pSuperblock->GetPayment(i, payment)) {
            // SET COINBASE OUTPUT TO SUPERBLOCK SETTING

            CTxOut txout = CTxOut(payment.nAmount, payment.script);
            voutSuperblockRet.push_back(txout);

            // PRINT NICE LOG OUTPUT FOR SUPERBLOCK PAYMENT

            CTxDestination address1;
            ExtractDestination(payment.script, address1);
            CBitcoinAddress address2(address1);

            // TODO: PRINT NICE N.N DASH OUTPUT

            LogPrint(BCLog::GOBJECT, "CSuperblockManager::GetSuperblockPayments -- NEW Superblock: output %d (addr %s, amount %lld)\n",
                        i, address2.ToString(), payment.nAmount);
        } else {
            LogPrint(BCLog::GOBJECT, "CSuperblockManager::GetSuperblockPayments -- Payment not found\n");
        }
    }

    return true;
}

bool CSuperblockManager::IsValid(const CTransaction& txNew, int nBlockHeight, CAmount blockReward)
{
    // GET BEST SUPERBLOCK, SHOULD MATCH
    LOCK(governance.cs);

    CSuperblock_sptr pSuperblock;
    if (CSuperblockManager::GetBestSuperblock(pSuperblock, nBlockHeight)) {
        return pSuperblock->IsValid(txNew, nBlockHeight, blockReward);
    }

    return false;
}

void CSuperblockManager::ExecuteBestSuperblock(int nBlockHeight)
{
    LOCK(governance.cs);

    CSuperblock_sptr pSuperblock;
    if (GetBestSuperblock(pSuperblock, nBlockHeight)) {
        // All checks are done in CSuperblock::IsValid via IsBlockValueValid and IsBlockPayeeValid,
        // tip wouldn't be updated if anything was wrong. Mark this trigger as executed.
        pSuperblock->SetExecuted();
    }
}

CSuperblock::
    CSuperblock() :
    nGovObjHash(),
    nBlockHeight(0),
    nStatus(SEEN_OBJECT_UNKNOWN),
    vecPayments()
{
}

CSuperblock::
    CSuperblock(uint256& nHash) :
    nGovObjHash(nHash),
    nBlockHeight(0),
    nStatus(SEEN_OBJECT_UNKNOWN),
    vecPayments()
{
    CGovernanceObject* pGovObj = GetGovernanceObject();

    if (!pGovObj) {
        throw std::runtime_error("CSuperblock: Failed to find Governance Object");
    }

    LogPrint(BCLog::GOBJECT, "CSuperblock -- Constructor pGovObj: %s, nObjectType = %d\n",
                pGovObj->GetDataAsPlainString(), pGovObj->GetObjectType());

    if (pGovObj->GetObjectType() != GOVERNANCE_OBJECT_TRIGGER) {
        throw std::runtime_error("CSuperblock: Governance Object not a trigger");
    }

    UniValue obj = pGovObj->GetJSONObject();

    // FIRST WE GET THE START HEIGHT, THE BLOCK HEIGHT AT WHICH THE PAYMENT SHALL OCCUR
    nBlockHeight = obj["event_block_height"].get_int();

    // NEXT WE GET THE PAYMENT INFORMATION AND RECONSTRUCT THE PAYMENT VECTOR
    std::string strAddresses = obj["payment_addresses"].get_str();
    std::string strAmounts = obj["payment_amounts"].get_str();
    ParsePaymentSchedule(strAddresses, strAmounts);

    LogPrint(BCLog::GOBJECT, "CSuperblock -- nBlockHeight = %d, strAddresses = %s, strAmounts = %s, vecPayments.size() = %d\n",
        nBlockHeight, strAddresses, strAmounts, vecPayments.size());
}

/**
 *   Is Valid Superblock Height
 *
 *   - See if a block at this height can be a superblock
 */

bool CSuperblock::IsValidBlockHeight(int nBlockHeight)
{
    // SUPERBLOCKS CAN HAPPEN ONLY after hardfork and only ONCE PER CYCLE
    return nBlockHeight >= Params().GetConsensus().nSuperblockStartBlock &&
           ((nBlockHeight % Params().GetConsensus().nSuperblockCycle) == 0);
}

void CSuperblock::GetNearestSuperblocksHeights(int nBlockHeight, int& nLastSuperblockRet, int& nNextSuperblockRet)
{
    const Consensus::Params& consensusParams = Params().GetConsensus();
    int nSuperblockStartBlock = consensusParams.nSuperblockStartBlock;
    int nSuperblockCycle = consensusParams.nSuperblockCycle;

    // Get first superblock
    int nFirstSuperblockOffset = (nSuperblockCycle - nSuperblockStartBlock % nSuperblockCycle) % nSuperblockCycle;
    int nFirstSuperblock = nSuperblockStartBlock + nFirstSuperblockOffset;

    if (nBlockHeight < nFirstSuperblock) {
        nLastSuperblockRet = 0;
        nNextSuperblockRet = nFirstSuperblock;
    } else {
        nLastSuperblockRet = nBlockHeight - nBlockHeight % nSuperblockCycle;
        nNextSuperblockRet = nLastSuperblockRet + nSuperblockCycle;
    }
}

CAmount CSuperblock::GetPaymentsLimit(int nBlockHeight)
{
    const Consensus::Params& consensusParams = Params().GetConsensus();

    if (!IsValidBlockHeight(nBlockHeight)) {
        return 0;
    }

    // min subsidy for high diff networks and vice versa
    int nBits = consensusParams.fPowAllowMinDifficultyBlocks ? UintToArith256(consensusParams.powLimit).GetCompact() : 1;
    // some part of all blocks issued during the cycle goes to superblock, see GetBlockSubsidy
    CAmount nSuperblockPartOfSubsidy = GetBlockSubsidy(nBits, nBlockHeight - 1, consensusParams, true);
    CAmount nPaymentsLimit = nSuperblockPartOfSubsidy * consensusParams.nSuperblockCycle;
    LogPrint(BCLog::GOBJECT, "CSuperblock::GetPaymentsLimit -- Valid superblock height %d, payments max %lld\n", nBlockHeight, nPaymentsLimit);

    return nPaymentsLimit;
}

void CSuperblock::ParsePaymentSchedule(const std::string& strPaymentAddresses, const std::string& strPaymentAmounts)
{
    // SPLIT UP ADDR/AMOUNT STRINGS AND PUT IN VECTORS

    std::vector<std::string> vecParsed1;
    std::vector<std::string> vecParsed2;
    vecParsed1 = SplitBy(strPaymentAddresses, "|");
    vecParsed2 = SplitBy(strPaymentAmounts, "|");

    // IF THESE DONT MATCH, SOMETHING IS WRONG

    if (vecParsed1.size() != vecParsed2.size()) {
        std::ostringstream ostr;
        ostr << "CSuperblock::ParsePaymentSchedule -- Mismatched payments and amounts";
        LogPrintf("%s\n", ostr.str());
        throw std::runtime_error(ostr.str());
    }

    if (vecParsed1.size() == 0) {
        std::ostringstream ostr;
        ostr << "CSuperblock::ParsePaymentSchedule -- Error no payments";
        LogPrintf("%s\n", ostr.str());
        throw std::runtime_error(ostr.str());
    }

    // LOOP THROUGH THE ADDRESSES/AMOUNTS AND CREATE PAYMENTS
    /*
      ADDRESSES = [ADDR1|2|3|4|5|6]
      AMOUNTS = [AMOUNT1|2|3|4|5|6]
    */

    for (int i = 0; i < (int)vecParsed1.size(); i++) {
        CBitcoinAddress address(vecParsed1[i]);
        if (!address.IsValid()) {
            std::ostringstream ostr;
            ostr << "CSuperblock::ParsePaymentSchedule -- Invalid Dash Address : " << vecParsed1[i];
            LogPrintf("%s\n", ostr.str());
            throw std::runtime_error(ostr.str());
        }
        /*
            TODO

            - There might be an issue with multisig in the coinbase on mainnet, we will add support for it in a future release.
            - Post 12.3+ (test multisig coinbase transaction)
        */
        if (address.IsScript()) {
            std::ostringstream ostr;
            ostr << "CSuperblock::ParsePaymentSchedule -- Script addresses are not supported yet : " << vecParsed1[i];
            LogPrintf("%s\n", ostr.str());
            throw std::runtime_error(ostr.str());
        }

        CAmount nAmount = ParsePaymentAmount(vecParsed2[i]);

        LogPrint(BCLog::GOBJECT, "CSuperblock::ParsePaymentSchedule -- i = %d, amount string = %s, nAmount = %lld\n", i, vecParsed2[i], nAmount);

        CGovernancePayment payment(address, nAmount);
        if (payment.IsValid()) {
            vecPayments.push_back(payment);
        } else {
            vecPayments.clear();
            std::ostringstream ostr;
            ostr << "CSuperblock::ParsePaymentSchedule -- Invalid payment found: address = " << address.ToString()
                 << ", amount = " << nAmount;
            LogPrintf("%s\n", ostr.str());
            throw std::runtime_error(ostr.str());
        }
    }
}

bool CSuperblock::GetPayment(int nPaymentIndex, CGovernancePayment& paymentRet)
{
    if ((nPaymentIndex < 0) || (nPaymentIndex >= (int)vecPayments.size())) {
        return false;
    }

    paymentRet = vecPayments[nPaymentIndex];
    return true;
}

CAmount CSuperblock::GetPaymentsTotalAmount()
{
    CAmount nPaymentsTotalAmount = 0;
    int nPayments = CountPayments();

    for (int i = 0; i < nPayments; i++) {
        nPaymentsTotalAmount += vecPayments[i].nAmount;
    }

    return nPaymentsTotalAmount;
}

/**
*   Is Transaction Valid
*
*   - Does this transaction match the superblock?
*/

bool CSuperblock::IsValid(const CTransaction& txNew, int nBlockHeight, CAmount blockReward)
{
    // TODO : LOCK(cs);
    // No reason for a lock here now since this method only accesses data
    // internal to *this and since CSuperblock's are accessed only through
    // shared pointers there's no way our object can get deleted while this
    // code is running.
    if (!IsValidBlockHeight(nBlockHeight)) {
        LogPrintf("CSuperblock::IsValid -- ERROR: Block invalid, incorrect block height\n");
        return false;
    }

    std::string strPayeesPossible = "";

    // CONFIGURE SUPERBLOCK OUTPUTS

    int nOutputs = txNew.vout.size();
    int nPayments = CountPayments();
    int nMinerAndMasternodePayments = nOutputs - nPayments;

    LogPrint(BCLog::GOBJECT, "CSuperblock::IsValid -- nOutputs = %d, nPayments = %d, GetDataAsHexString = %s\n",
        nOutputs, nPayments, GetGovernanceObject()->GetDataAsHexString());

    // We require an exact match (including order) between the expected
    // superblock payments and the payments actually in the block.

    if (nMinerAndMasternodePayments < 0) {
        // This means the block cannot have all the superblock payments
        // so it is not valid.
        // TODO: could that be that we just hit coinbase size limit?
        LogPrintf("CSuperblock::IsValid -- ERROR: Block invalid, too few superblock payments\n");
        return false;
    }

    // payments should not exceed limit
    CAmount nPaymentsTotalAmount = GetPaymentsTotalAmount();
    CAmount nPaymentsLimit = GetPaymentsLimit(nBlockHeight);
    if (nPaymentsTotalAmount > nPaymentsLimit) {
        LogPrintf("CSuperblock::IsValid -- ERROR: Block invalid, payments limit exceeded: payments %lld, limit %lld\n", nPaymentsTotalAmount, nPaymentsLimit);
        return false;
    }

    // miner and masternodes should not get more than they would usually get
    CAmount nBlockValue = txNew.GetValueOut();
    if (nBlockValue > blockReward + nPaymentsTotalAmount) {
        LogPrintf("CSuperblock::IsValid -- ERROR: Block invalid, block value limit exceeded: block %lld, limit %lld\n", nBlockValue, blockReward + nPaymentsTotalAmount);
        return false;
    }

    int nVoutIndex = 0;
    for (int i = 0; i < nPayments; i++) {
        CGovernancePayment payment;
        if (!GetPayment(i, payment)) {
            // This shouldn't happen so log a warning
            LogPrintf("CSuperblock::IsValid -- WARNING: Failed to find payment: %d of %d total payments\n", i, nPayments);
            continue;
        }

        bool fPaymentMatch = false;

        for (int j = nVoutIndex; j < nOutputs; j++) {
            // Find superblock payment
            fPaymentMatch = ((payment.script == txNew.vout[j].scriptPubKey) &&
                             (payment.nAmount == txNew.vout[j].nValue));

            if (fPaymentMatch) {
                nVoutIndex = j;
                break;
            }
        }

        if (!fPaymentMatch) {
            // Superblock payment not found!

            CTxDestination address1;
            ExtractDestination(payment.script, address1);
            CBitcoinAddress address2(address1);
            LogPrintf("CSuperblock::IsValid -- ERROR: Block invalid: %d payment %d to %s not found\n", i, payment.nAmount, address2.ToString());

            return false;
        }
    }

    return true;
}

bool CSuperblock::IsExpired()
{
    bool fExpired{false};
    int nExpirationBlocks{0};
    // Executed triggers are kept for another superblock cycle (approximately 1 month),
    // other valid triggers are kept for ~1 day only, everything else is pruned after ~1h.
    switch (nStatus) {
    case SEEN_OBJECT_EXECUTED:
        nExpirationBlocks = Params().GetConsensus().nSuperblockCycle;
        break;
    case SEEN_OBJECT_IS_VALID:
        nExpirationBlocks = 576;
        break;
    default:
        nExpirationBlocks = 24;
        break;
    }

    int nExpirationBlock = nBlockHeight + nExpirationBlocks;

    LogPrint(BCLog::GOBJECT, "CSuperblock::IsExpired -- nBlockHeight = %d, nExpirationBlock = %d\n", nBlockHeight, nExpirationBlock);

    if (governance.GetCachedBlockHeight() > nExpirationBlock) {
        LogPrint(BCLog::GOBJECT, "CSuperblock::IsExpired -- Outdated trigger found\n");
        fExpired = true;
        CGovernanceObject* pgovobj = GetGovernanceObject();
        if (pgovobj) {
            LogPrint(BCLog::GOBJECT, "CSuperblock::IsExpired -- Expiring outdated object: %s\n", pgovobj->GetHash().ToString());
            pgovobj->fExpired = true;
            pgovobj->nDeletionTime = GetAdjustedTime();
        }
    }

    return fExpired;
}

/**
*   Get Required Payment String
*
*   - Get a string representing the payments required for a given superblock
*/

std::string CSuperblockManager::GetRequiredPaymentsString(int nBlockHeight)
{
    LOCK(governance.cs);
    std::string ret = "Unknown";

    // GET BEST SUPERBLOCK

    CSuperblock_sptr pSuperblock;
    if (!GetBestSuperblock(pSuperblock, nBlockHeight)) {
        LogPrint(BCLog::GOBJECT, "CSuperblockManager::GetRequiredPaymentsString -- Can't find superblock for height %d\n", nBlockHeight);
        return "error";
    }

    // LOOP THROUGH SUPERBLOCK PAYMENTS, CONFIGURE OUTPUT STRING

    for (int i = 0; i < pSuperblock->CountPayments(); i++) {
        CGovernancePayment payment;
        if (pSuperblock->GetPayment(i, payment)) {
            // PRINT NICE LOG OUTPUT FOR SUPERBLOCK PAYMENT

            CTxDestination address1;
            ExtractDestination(payment.script, address1);
            CBitcoinAddress address2(address1);

            // RETURN NICE OUTPUT FOR CONSOLE

            if (ret != "Unknown") {
                ret += ", " + address2.ToString();
            } else {
                ret = address2.ToString();
            }
        }
    }

    return ret;
}

CProposalDetail::CProposalDetail(const std::string& strDataHex):
    nStartEpoch(0),
    nEndEpoch(0),
    nStartHeight(0),
    nEndHeight(0),
    nPaymentAmount(0),
    fOldFormat(false),
    fParsedOK(false)
{
    if (!strDataHex.empty()) {
        ParseStrDataHex(strDataHex);
    }
}

void CProposalDetail::ParseStrDataHex(const std::string& strDataHex)
{
    UniValue obj(UniValue::VOBJ);
    std::vector<unsigned char> vchData = ParseHex(strDataHex);
    try {
        obj.read(std::string(vchData.begin(), vchData.end()));
        if (!obj.isObject()) {
            throw std::runtime_error("Parse error - object expected");
        }

        // Make a map of all obj keys, so we can call hasKey().
        std::map<std::string, bool> mapObjKeys;
        for (auto key : obj.getKeys()) {
            mapObjKeys.emplace(key, true);
        }
        // Define a hasKey lambda function.
        auto hasKey = [&mapObjKeys](const std::string& key) { return (mapObjKeys.count(key) > 0); };

        // Load data members from obj

        // Parse proposal details
        if (hasKey("startHeight")) {
            nStartHeight = obj["startHeight"].get_int();
            nEndEpoch = obj["endHeight"].get_int();
            strName = obj["name"].get_str();
            strURL = obj["url"].get_str();
            payeeAddr = CBitcoinAddress(obj["payment_address"].get_str());
            // This new format is already in Satoshis
            nPaymentAmount = int64_t(obj["payment_amount"].get_int64());
        } else if (hasKey("start_epoch")) {
            // Do it the old way.
            nStartEpoch = obj["start_epoch"].get_int();
            nEndEpoch = obj["end_epoch"].get_int();
            strName = obj["name"].get_str();
            strURL = obj["url"].get_str();
            payeeAddr = CBitcoinAddress(obj["payment_address"].get_str());
            // This old format is in COIN - convert to Satoshis
            nPaymentAmount = int64_t(obj["payment_amount"].get_real() * COIN);
            fOldFormat = true;
        }

        fParsedOK = true;
    } catch (std::exception& e) {
        LogPrint("gobject", "NGM shoot, failed: %s\n", std::string(e.what()));
        vecStrErrMessages.emplace_back(std::string(e.what()));
    } catch (...) {
        LogPrint("gobject", "NGM shoot, unknown exception\n");
        vecStrErrMessages.emplace_back("Unknown exception");
    }
}

uint256 CProposalDetail::GetHash() const
{
    CHashWriter ss(SER_GETHASH, 0);

    ss << strName;
    ss << strURL;
    ss << payeeAddr.ToString();
    ss << nPaymentAmount;
    ss << nStartEpoch;
    ss << nEndEpoch;

    return ss.GetHash();
}

std::string CProposalDetail::ErrorMessages() const
{
    std::string combinedMessage;
    for (std::string msg : vecStrErrMessages) {
        if (!combinedMessage.empty()) combinedMessage += ";";
        combinedMessage += msg;
    }
    return combinedMessage;
}

CTriggerDetail::CTriggerDetail(const std::string& strDataHex):
    nHeight(0),
    fParsedOK(false)
{
    if (!strDataHex.empty()) {
        ParseStrDataHex(strDataHex);
    }
}

CTriggerDetail::CTriggerDetail(int nHeight, const std::vector<const CGovernanceObject *>& vecProposals):
    nHeight(nHeight),
    fParsedOK(false)
{
    for (const auto& pGovObj : vecProposals) {
        uint256 hash = pGovObj->GetHash();

        auto detail = CProposalDetail(pGovObj->GetDataAsHexString());
        if (!detail.DidParse()) {
            vecStrErrMessages.emplace_back(detail.ErrorMessages());
            return;
        }
        vecPayments.push_back(CPayment(hash, detail.Address(), detail.Amount()));
    }

    // Order payments once loaded, sorted by proposal hash descending
    std::sort(vecPayments.rbegin(), vecPayments.rend());

    fParsedOK = true;
}

void CTriggerDetail::ParseStrDataHex(const std::string& strDataHex)
{
    UniValue obj(UniValue::VOBJ);
    std::vector<unsigned char> vchData = ParseHex(strDataHex);
    try {
        obj.read(std::string(vchData.begin(), vchData.end()));
        if (!obj.isObject()) {
            throw std::runtime_error("Parse error - object expected");
        }

        // Make a map of all obj keys, so we can call hasKey().
        std::map<std::string, bool> mapObjKeys;
        for (auto key : obj.getKeys()) {
            mapObjKeys.emplace(key, true);
        }
        // Define a hasKey lambda function.
        auto hasKey = [&mapObjKeys](const std::string& key) { return (mapObjKeys.count(key) > 0); };

        // Load data members from obj

        // Superblock Height
        if (hasKey("sbHeight")) {
            nHeight = obj["sbHeight"].get_int();
        } else if (hasKey("event_block_height")) {
            nHeight = obj["event_block_height"].get_int();
        }

        // Payments data
        if (hasKey("payments")) {
            const UniValue& payments = obj["payments"].get_array();
            for (size_t i = 0; i < payments.size(); ++i) {
                const UniValue& pymtObj = payments[i];
                CBitcoinAddress address(pymtObj["address"].get_str());
                CAmount nAmount(pymtObj["amount"].get_int64());
                uint256 hash = uint256S(pymtObj["propHash"].get_str());
                vecPayments.push_back(CPayment(hash, address, nAmount));
            }
        } else {
            // Do it the less civilized way.
            //
            // All this rubbish (this else branch) can hopefully be deleted
            // once objects move over to the new format.
            //
            std::string strPaymentAddresses = obj["payment_addresses"].get_str();
            std::string strPaymentAmounts = obj["payment_amounts"].get_str();
            std::string strProposalHashes = obj["proposal_hashes"].get_str();

            std::vector<std::string> vecAddrs = SplitBy(strPaymentAddresses, "|");
            std::vector<std::string> vecAmts = SplitBy(strPaymentAmounts, "|");
            std::vector<std::string> vecHashes = SplitBy(strProposalHashes, "|");

            bool fSameSize = (vecAddrs.size() == vecAmts.size() && vecAmts.size() == vecHashes.size());

            // If the sizes for each of the three vectors do not match, something is wrong.
            if (!fSameSize) {
                std::ostringstream ostr;
                ostr << __func__ << " - Mismatched payments, amounts, and/or proposal hashes";
                LogPrint("gobject", "%s\n", ostr.str());
                throw std::runtime_error(ostr.str());
            }

            for (int q = 0; q < (int)vecAddrs.size(); ++q) {
                CBitcoinAddress address(vecAddrs[q]);
                CAmount nAmount = ParsePaymentAmount(vecAmts[q]);
                uint256 hash = uint256S(vecHashes[q]);
                vecPayments.push_back(CPayment(hash, address, nAmount));
            }
        }

        // Order payments once parsed, sorted by proposal hash descending
        std::sort(vecPayments.rbegin(), vecPayments.rend());

        fParsedOK = true;
    } catch (std::exception& e) {
        LogPrint("gobject", "NGM shoot, failed: %s\n", std::string(e.what()));
        vecStrErrMessages.emplace_back(std::string(e.what()));
    } catch (...) {
        LogPrint("gobject", "NGM shoot, unknown exception\n");
        vecStrErrMessages.emplace_back("Unknown exception");
    }
}

uint256 CTriggerDetail::GetHash() const
{
    CHashWriter ss(SER_GETHASH, 0);
    ss << nHeight;

    // Add each payment to the stream
    for (const auto& p : vecPayments) {
        ss << p;
    }

    return ss.GetHash();
}

std::string CTriggerDetail::ErrorMessages() const
{
    std::string combinedMessage;
    for (std::string msg : vecStrErrMessages) {
        if (!combinedMessage.empty()) combinedMessage += ";";
        combinedMessage += msg;
    }
    return combinedMessage;
}

std::string CTriggerDetail::GetDataHexStr() const
{
    UniValue objJSON(UniValue::VOBJ);
    objJSON.push_back(Pair("sbHeight", nHeight));
    objJSON.push_back(Pair("type", 2));

    UniValue arrPaymentsJSON(UniValue::VARR);
    for (const auto& p : vecPayments) {
        UniValue objPayment(UniValue::VOBJ);

        objPayment.push_back(Pair("address", p.address.ToString()));
        objPayment.push_back(Pair("amount", p.nAmount));
        objPayment.push_back(Pair("propHash", p.nProposalHash.ToString()));

        arrPaymentsJSON.push_back(objPayment);
    }
    objJSON.push_back(Pair("payments", arrPaymentsJSON));

    std::string strValue = objJSON.write(0, 1);
    std::string strHexValue = HexStr(strValue);

    return strHexValue;
}

CPayment::CPayment(const uint256& nProposalHash, CBitcoinAddress address, CAmount nAmount) :
    nProposalHash(nProposalHash),
    address(address),
    nAmount(nAmount)
{ }
