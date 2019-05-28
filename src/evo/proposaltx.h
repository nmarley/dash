// Copyright (c) 2019 The Dash Core developers
// Distributed under the MIT software license, see the accompanying
// file COPYING or http://www.opensource.org/licenses/mit-license.php.

#ifndef DASH_PROPOSALTX_H
#define DASH_PROPOSALTX_H

#include "consensus/validation.h"
#include "primitives/transaction.h"

class CBlock;
class CBlockIndex;
class UniValue;

// Transaction with governance proposal payload
class CProposalTx
{
public:
    static const uint16_t CURRENT_VERSION = 1;

    uint16_t nVersion{CURRENT_VERSION};
    int32_t nHeight{0};

    // payload data
    int32_t nStartHeight{0};
    int32_t nEndHeight{0};
    CAmount nAmount{0};
    CScript scriptPayout;

    ADD_SERIALIZE_METHODS;

    template <typename Stream, typename Operation>
    inline void SerializationOp(Stream& s, Operation ser_action)
    {
        READWRITE(nVersion);
        READWRITE(nHeight);
        READWRITE(nStartHeight);
        READWRITE(nEndHeight);
    }

    std::string ToString() const;
    void ToJson(UniValue& obj) const;
};

bool CheckProposalTx(const CTransaction& tx, const CBlockIndex* pindexPrev, CValidationState& state);

#endif //DASH_PROPOSALTX_H
