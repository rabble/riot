require 'tzinfo/timezone_definition'

module TZInfo
  module Definitions
    module UYT
      include TimezoneDefinition
      
      linked_timezone 'UYT', 'America/Montevideo'
    end
  end
end
