// Copyright (c) 2014-2017 The Dash Core developers
// Distributed under the MIT/X11 software license, see the accompanying
// file COPYING or http://www.opensource.org/licenses/mit-license.php.

#ifndef GOVERNANCE_VALIDATORS_H
#define GOVERNANCE_VALIDATORS_H

#include <string>

#include <univalue.h>

// for governance object type constants
#include "governance-object.h"

// or should these be pulled from files instead?
static const std::string TRIGGER_SCHEMA_V1 = std::string(R"({
  "$schema": "http://json-schema.org/draft-04/schema#",
  "title": "Trigger",
  "type": "object",
  "properties": {
    "type": {
      "type": "integer",
        "minimum": 2,
        "maximum": 2
    },
    "event_block_height": {
      "type": "integer"
    },
    "payment_addresses": {
      "type": "string"
    },
    "payment_amounts": {
      "type": "string"
    }
  },
  "required": ["type", "event_block_height", "payment_addresses", "payment_amounts"]
})");

static const std::string PROPOSAL_SCHEMA_V1 = std::string(R"({
  "$schema": "http://json-schema.org/draft-04/schema#",
  "title": "Proposal",
  "type": "object",
  "properties": {
    "type": {
      "type": "integer",
      "minimum": 1,
      "maximum": 1
    },
    "name": {
      "type": "string"
    },
    "url": {
      "type": "string",
      "format": "uri"
    },
    "payment_address": {
      "type": "string"
    },
    "payment_amount": {
      "type": "number",
      "minimum": 0.00000001,
      "maximum": 6652.0
    },
    "start_epoch": {
      "type": "integer",
      "minimum": 0,
      "maximum": 4294967295
    },
    "end_epoch": {
      "type": "integer",
      "minimum": 0,
      "maximum": 4294967295
    }
  },
  "required": ["type", "name", "url", "payment_address", "payment_amount", "start_epoch", "end_epoch"]
})");

class CProposalValidator
{
private:
    UniValue               objJSON;
    bool                   fJSONValid;
    std::string            strErrorMessages;

public:
    CProposalValidator(const std::string& strDataHexIn = std::string());

    bool Validate();

    const std::string& GetErrorMessages()
    {
        return strErrorMessages;
    }

    bool GetJSONSchemaForObjectType(const int nObjectType, std::string& strValue);

private:
    void ParseStrHexData(const std::string& strHexData);
    void ParseJSONData(const std::string& strJSONData);

    bool GetDataValue(const std::string& strKey, std::string& strValueRet);
    bool GetDataValue(const std::string& strKey, int64_t& nValueRet);
    bool GetDataValue(const std::string& strKey, double& dValueRet);

    bool ValidateName();
    bool ValidateStartEndEpoch();
    bool ValidatePaymentAmount();
    bool ValidatePaymentAddress();
    bool ValidateURL();

    bool CheckURL(const std::string& strURLIn);
};

#endif
